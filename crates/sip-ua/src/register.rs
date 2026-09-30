use std::net::SocketAddr;
use std::sync::Arc;
use tokio::time::{Duration, Instant};
use tracing::{info, warn};

use crate::auth::create_auth_response;
use crate::error::{Result, UaError};
use sip_core::{
    CSeqHeader, Header, Method, NameAddr, Request, Response, SipUri, Uri, parse_name_addr,
};
use sip_transaction::TransactionManager;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountConfig {
    pub username: String,
    pub password: String,
    pub domain: String,
    pub registrar_addr: SocketAddr,
    pub expires: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationState {
    Unregistered,
    Registering,
    Registered { expires_at: Instant },
    Failed(String),
}

pub struct RegistrationClient {
    pub config: AccountConfig,
    pub contact_uri: SipUri,
    pub state: RegistrationState,
    cseq: u32,
    call_id: String,
    local_tag: String,
    tx_manager: Arc<TransactionManager>,
}

impl RegistrationClient {
    pub fn new(
        config: AccountConfig,
        contact_uri: SipUri,
        tx_manager: Arc<TransactionManager>,
    ) -> Self {
        let call_id = format!(
            "reg-{}@{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            config.domain
        );
        let local_tag = format!(
            "rtg{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        );

        Self {
            config,
            contact_uri,
            state: RegistrationState::Unregistered,
            cseq: 1,
            call_id,
            local_tag,
            tx_manager,
        }
    }

    pub async fn register(&mut self) -> Result<u32> {
        self.state = RegistrationState::Registering;

        let req = self.build_register_request(None, self.config.expires);
        info!("Sending REGISTER to {}", self.config.registrar_addr);

        let mut handle = self
            .tx_manager
            .send_request(req, self.config.registrar_addr)
            .await?;

        let res = handle
            .recv_response()
            .await
            .ok_or(UaError::RegistrationFailed("No response received".into()))?;

        if res.is_success() {
            let expires = self.extract_expires(&res);
            self.state = RegistrationState::Registered {
                expires_at: Instant::now() + Duration::from_secs(expires as u64),
            };
            info!("Registered successfully! Expires in {}s", expires);
            return Ok(expires);
        }

        // Handle 401 Unauthorized or 407 Proxy Authentication Required
        if res.status_code == 401 || res.status_code == 407 {
            info!(
                "Received {} challenge, computing digest...",
                res.status_code
            );
            let challenge = res
                .headers
                .iter()
                .find_map(|h| match h {
                    Header::WwwAuthenticate(a) => Some(a),
                    Header::ProxyAuthenticate(a) => Some(a),
                    _ => None,
                })
                .ok_or(UaError::AuthFailed("No challenge header in 401/407".into()))?;

            let digest_uri = format!("sip:{}", self.config.domain);
            let auth_header = create_auth_response(
                challenge,
                &self.config.username,
                &self.config.password,
                "REGISTER",
                &digest_uri,
            );

            // Re-send with authorization header
            let auth_req = self.build_register_request(Some(auth_header), self.config.expires);
            let mut auth_handle = self
                .tx_manager
                .send_request(auth_req, self.config.registrar_addr)
                .await?;

            let auth_res = auth_handle
                .recv_response()
                .await
                .ok_or(UaError::RegistrationFailed(
                    "No response after authentication".into(),
                ))?;

            if auth_res.is_success() {
                let expires = self.extract_expires(&auth_res);
                self.state = RegistrationState::Registered {
                    expires_at: Instant::now() + Duration::from_secs(expires as u64),
                };
                info!(
                    "Authenticated registration successful! Expires in {}s",
                    expires
                );
                return Ok(expires);
            } else {
                let err_msg = format!(
                    "Auth failed with status {}: {}",
                    auth_res.status_code, auth_res.reason_phrase
                );
                warn!("{}", err_msg);
                self.state = RegistrationState::Failed(err_msg.clone());
                return Err(UaError::RegistrationFailed(err_msg));
            }
        }

        let err_msg = format!(
            "Registration rejected: {} {}",
            res.status_code, res.reason_phrase
        );
        warn!("{}", err_msg);
        self.state = RegistrationState::Failed(err_msg.clone());
        Err(UaError::RegistrationFailed(err_msg))
    }

    pub async fn unregister(&mut self) -> Result<()> {
        let req = self.build_register_request(None, 0);
        let mut handle = self
            .tx_manager
            .send_request(req, self.config.registrar_addr)
            .await?;

        let _ = handle.recv_response().await;
        self.state = RegistrationState::Unregistered;
        info!("Unregistered");
        Ok(())
    }

    fn build_register_request(
        &mut self,
        auth_header: Option<sip_core::AuthHeader>,
        expires: u32,
    ) -> Request {
        self.cseq += 1;

        let request_uri = Uri::Sip(SipUri::new(&self.config.domain));
        let from_str = format!(
            "<sip:{}@{}>;tag={}",
            self.config.username, self.config.domain, self.local_tag
        );
        let to_str = format!("<sip:{}@{}>", self.config.username, self.config.domain);

        let mut req = Request::new(Method::Register, request_uri);
        req.headers
            .push(Header::From(parse_name_addr(&from_str).unwrap()));
        req.headers
            .push(Header::To(parse_name_addr(&to_str).unwrap()));
        req.headers.push(Header::CallId(self.call_id.clone()));
        req.headers
            .push(Header::CSeq(CSeqHeader::new(self.cseq, Method::Register)));
        req.headers.push(Header::MaxForwards(70));
        req.headers.push(Header::Expires(expires));

        // Contact with expires param
        let mut contact = NameAddr::new(Uri::Sip(self.contact_uri.clone()));
        contact
            .parameters
            .push(("expires".to_string(), Some(expires.to_string())));
        req.headers.push(Header::Contact(contact));

        if let Some(auth) = auth_header {
            req.headers.push(Header::Authorization(auth));
        }

        req.headers.push(Header::ContentLength(0));
        req
    }

    fn extract_expires(&self, res: &Response) -> u32 {
        if let Some(exp) = res.headers.expires() {
            return exp;
        }
        if let Some(exp) = res
            .headers
            .first_contact()
            .and_then(|c| c.get_param("expires").flatten())
            .and_then(|exp_str| exp_str.parse::<u32>().ok())
        {
            return exp;
        }
        self.config.expires
    }
}
