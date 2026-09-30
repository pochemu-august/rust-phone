use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use tokio::sync::{Mutex, mpsc};
use tracing::trace;

use sdp::SessionDescription;
use sip_core::prelude::*;
use sip_transaction::prelude::*;
use sip_transport::TransportManager;
use sip_ua::prelude::*;

use crate::account::Account;
use crate::call::{Call, CallDirection};
use crate::config::EndpointConfig;
use crate::error::{PjsuaError, Result};
use crate::event::{CallState, PjsuaEvent};
use crate::media_session::{AudioCodecType, MediaSession};
use crate::sdp_helper::{create_audio_answer, create_audio_offer, extract_media_socket_addr};

pub struct Endpoint {
    config: EndpointConfig,
    transport: Arc<TransportManager>,
    transactions: Arc<TransactionManager>,
    dialogs: Arc<DialogManager>,
    accounts: Arc<Mutex<Vec<Arc<Account>>>>,
    calls: Arc<Mutex<HashMap<String, Arc<Call>>>>,
    server_txs: Arc<Mutex<HashMap<String, Arc<ServerTransaction>>>>,
    event_tx: mpsc::UnboundedSender<PjsuaEvent>,
    event_rx: Arc<Mutex<mpsc::UnboundedReceiver<PjsuaEvent>>>,
    next_rtp_port: Arc<AtomicU16>,
}

impl Endpoint {
    pub async fn init(config: EndpointConfig) -> Result<Arc<Self>> {
        let (incoming_sip_tx, incoming_sip_rx) = mpsc::channel(128);
        let transport =
            Arc::new(TransportManager::new_udp(config.sip_bind_addr, incoming_sip_tx).await?);

        let (server_tx_dispatcher, mut server_tx_rx) = mpsc::channel(64);
        let timers = SipTimers::default();
        let transactions = TransactionManager::new(
            Arc::clone(&transport),
            incoming_sip_rx,
            server_tx_dispatcher,
            timers,
        );

        let (ack_tx, mut ack_rx) = mpsc::channel(64);
        transactions.set_ack_dispatcher(ack_tx).await;

        let dialogs = Arc::new(DialogManager::new());
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let rtp_port_start = config.rtp_port_start;

        let endpoint = Arc::new(Self {
            config,
            transport,
            transactions,
            dialogs,
            accounts: Arc::new(Mutex::new(Vec::new())),
            calls: Arc::new(Mutex::new(HashMap::new())),
            server_txs: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
            next_rtp_port: Arc::new(AtomicU16::new(rtp_port_start)),
        });

        // Background loop for incoming ServerTransactions (INVITE, BYE, OPTIONS, etc.)
        let ep_server = Arc::clone(&endpoint);
        tokio::spawn(async move {
            while let Some(server_tx) = server_tx_rx.recv().await {
                ep_server.handle_incoming_server_tx(server_tx).await;
            }
        });

        // Background loop for 2xx ACK requests
        let ep_ack = Arc::clone(&endpoint);
        tokio::spawn(async move {
            while let Some(ack_req) = ack_rx.recv().await {
                ep_ack.handle_incoming_ack(ack_req).await;
            }
        });

        Ok(endpoint)
    }

    pub fn local_sip_addr(&self) -> SocketAddr {
        self.transport.local_addr()
    }

    pub fn next_rtp_port(&self) -> u16 {
        self.next_rtp_port.fetch_add(2, Ordering::SeqCst)
    }

    pub async fn next_event(&self) -> Option<PjsuaEvent> {
        self.event_rx.lock().await.recv().await
    }

    pub async fn add_account(&self, config: AccountConfig) -> usize {
        let mut accs = self.accounts.lock().await;
        let id = accs.len();
        let contact = SipUri::new(self.transport.local_addr().ip().to_string())
            .with_port(self.transport.local_addr().port());
        let acc = Arc::new(Account::new(
            id,
            config,
            contact,
            Arc::clone(&self.transactions),
        ));
        accs.push(acc);
        id
    }

    pub async fn get_account(&self, acc_id: usize) -> Option<Arc<Account>> {
        self.accounts.lock().await.get(acc_id).cloned()
    }

    pub async fn get_call(&self, call_id: &str) -> Option<Arc<Call>> {
        self.calls.lock().await.get(call_id).cloned()
    }

    pub async fn register_account(&self, acc_id: usize) -> Result<u32> {
        let account = self
            .get_account(acc_id)
            .await
            .ok_or(PjsuaError::AccountNotFound(acc_id))?;

        let expires = account.register().await?;
        let state = account.state().await;
        let _ = self.event_tx.send(PjsuaEvent::RegistrationStateChanged {
            acc_id,
            state,
            status_code: 200,
            reason: "OK".to_string(),
        });

        Ok(expires)
    }

    pub async fn make_call(&self, acc_id: usize, dst_uri_str: &str) -> Result<Arc<Call>> {
        let account = self
            .get_account(acc_id)
            .await
            .ok_or(PjsuaError::AccountNotFound(acc_id))?;

        let dst_uri = parse_uri(dst_uri_str)?;
        let call_id = format!("call-{}", rand_u32());
        let local_tag = format!("tag-{}", rand_u32());

        // Allocate RTP port & create local MediaSession
        let local_rtp_port = self.next_rtp_port();
        let rtp_bind_addr = SocketAddr::new(self.config.rtp_bind_ip, local_rtp_port);
        let media_session = MediaSession::create(rtp_bind_addr, AudioCodecType::Pcmu).await?;

        // Generate SDP offer
        let sdp_offer = create_audio_offer(self.config.rtp_bind_ip, local_rtp_port, rand_u64());
        let sdp_str = sdp_offer.to_string();

        let mut req = Request::new(Method::Invite, dst_uri.clone());
        let from_uri = Uri::Sip(
            SipUri::new(account.config().domain.clone())
                .with_user(account.config().username.clone()),
        );
        let from_na = NameAddr::new(from_uri).with_param("tag", Some(local_tag));
        let to_na = NameAddr::new(dst_uri.clone());

        req.headers.push(Header::From(from_na.clone()));
        req.headers.push(Header::To(to_na.clone()));
        req.headers.push(Header::CallId(call_id.clone()));
        req.headers
            .push(Header::CSeq(CSeqHeader::new(1, Method::Invite)));
        req.headers.push(Header::MaxForwards(70));

        let local_contact = Uri::Sip(
            SipUri::new(self.transport.local_addr().ip().to_string())
                .with_port(self.transport.local_addr().port()),
        );
        req.headers
            .push(Header::Contact(NameAddr::new(local_contact)));
        req.headers
            .push(Header::ContentType("application/sdp".to_string()));
        req.headers.push(Header::Generic(
            "User-Agent".to_string(),
            self.config.user_agent.clone(),
        ));
        req.body = sdp_str.into_bytes();
        req.headers.push(Header::ContentLength(req.body.len()));

        let call = Arc::new(Call::new(
            call_id.clone(),
            CallDirection::Outgoing,
            from_na.to_string(),
            to_na.to_string(),
            CallState::Calling,
        ));

        call.set_local_sdp(sdp_offer).await;
        call.set_media_session(media_session).await;

        self.calls
            .lock()
            .await
            .insert(call_id.clone(), Arc::clone(&call));

        // Resolve destination socket address
        let target_host = match &dst_uri {
            Uri::Sip(s) => s.host.clone(),
            _ => "127.0.0.1".to_string(),
        };
        let target_port = match &dst_uri {
            Uri::Sip(s) => s.port.unwrap_or(5060),
            _ => 5060,
        };
        let target: SocketAddr =
            format!("{}:{}", target_host, target_port)
                .parse()
                .map_err(|_| {
                    PjsuaError::InvalidDestination(format!("{}:{}", target_host, target_port))
                })?;

        let mut handle = self.transactions.send_request(req.clone(), target).await?;
        let event_tx = self.event_tx.clone();
        let call_clone = Arc::clone(&call);
        let dialogs_clone = Arc::clone(&self.dialogs);
        let transport_clone = Arc::clone(&self.transport);

        tokio::spawn(async move {
            while let Some(res) = handle.recv_response().await {
                trace!(
                    "Call {} received INVITE response {}",
                    call_id, res.status_code
                );
                if res.status_code < 200 {
                    call_clone.set_state(CallState::Early).await;
                    let _ = event_tx.send(PjsuaEvent::CallStateChanged {
                        call_id: call_id.clone(),
                        state: CallState::Early,
                        status_code: res.status_code,
                        reason: res.reason_phrase.clone(),
                    });
                } else if res.status_code >= 200 && res.status_code < 300 {
                    // Success! 200 OK
                    call_clone.set_state(CallState::Connecting).await;

                    // Parse SDP answer if present
                    if let Ok(body_str) = std::str::from_utf8(&res.body)
                        && let Ok(sdp_ans) = SessionDescription::from_str(body_str)
                        && let Some(remote_rtp) = extract_media_socket_addr(&sdp_ans)
                    {
                        trace!("Call {} remote RTP endpoint: {}", call_id, remote_rtp);
                        call_clone.set_remote_sdp(sdp_ans).await;
                        call_clone.set_remote_rtp_addr(remote_rtp).await;
                        let _ = event_tx.send(PjsuaEvent::MediaStreamReady {
                            call_id: call_id.clone(),
                            local_addr: target,
                            remote_addr: remote_rtp,
                            codec: "PCMU/8000".to_string(),
                        });
                    }

                    // Create Dialog & Send 2xx ACK
                    if let Ok(dialog) = Dialog::create_uac(&req, &res) {
                        if let Ok(mut ack_req) = dialog.create_ack_for_2xx(&res) {
                            let branch = format!("z9hG4bK-{}", rand_u32());
                            let via = ViaHeader::new(
                                "SIP/2.0/UDP",
                                transport_clone.local_addr().ip().to_string(),
                                Some(transport_clone.local_addr().port()),
                            )
                            .with_branch(branch);
                            ack_req.headers.insert(0, Header::Via(via));

                            trace!("Sending 2xx ACK for call {}", call_id);
                            let _ = transport_clone
                                .send_to(&SipMessage::Request(ack_req), target)
                                .await;
                        }
                        dialogs_clone.insert(dialog.clone()).await;
                        call_clone.set_dialog(dialog).await;
                    }

                    call_clone.set_state(CallState::Confirmed).await;
                    let _ = event_tx.send(PjsuaEvent::CallStateChanged {
                        call_id: call_id.clone(),
                        state: CallState::Confirmed,
                        status_code: res.status_code,
                        reason: res.reason_phrase.clone(),
                    });
                    break;
                } else {
                    // 3xx-6xx Failure
                    call_clone.set_state(CallState::Disconnected).await;
                    let _ = event_tx.send(PjsuaEvent::CallStateChanged {
                        call_id: call_id.clone(),
                        state: CallState::Disconnected,
                        status_code: res.status_code,
                        reason: res.reason_phrase.clone(),
                    });
                    break;
                }
            }
        });

        Ok(call)
    }

    pub async fn answer_call(&self, call_id: &str, status_code: u16) -> Result<()> {
        let call = self
            .get_call(call_id)
            .await
            .ok_or_else(|| PjsuaError::CallNotFound(call_id.to_string()))?;

        let server_tx = self
            .server_txs
            .lock()
            .await
            .get(call_id)
            .cloned()
            .ok_or_else(|| PjsuaError::CallNotFound(call_id.to_string()))?;

        if status_code == 200 {
            // Bind RTP MediaSession
            let local_rtp_port = self.next_rtp_port();
            let rtp_bind_addr = SocketAddr::new(self.config.rtp_bind_ip, local_rtp_port);
            let mut media_session =
                MediaSession::create(rtp_bind_addr, AudioCodecType::Pcmu).await?;

            // Generate SDP Answer
            let remote_sdp = call.get_remote_sdp().await;
            let remote_rtp = remote_sdp.as_ref().and_then(extract_media_socket_addr);

            if let Some(r_addr) = remote_rtp {
                media_session.set_remote_addr(r_addr);
            }

            let sdp_ans =
                create_audio_answer(self.config.rtp_bind_ip, local_rtp_port, rand_u64(), "0");
            let sdp_str = sdp_ans.to_string();

            let local_tag = format!("tag-{}", rand_u32());
            let mut res = Response::new(200, "OK");

            // Copy required headers from INVITE request
            if let Some(via) = server_tx.request.headers.first_via() {
                res.headers.push(Header::Via(via.clone()));
            }
            if let Some(from) = server_tx.request.headers.from() {
                res.headers.push(Header::From(from.clone()));
            }
            if let Some(to) = server_tx.request.headers.to() {
                let mut to_header = to.clone();
                to_header
                    .parameters
                    .push(("tag".to_string(), Some(local_tag.clone())));
                res.headers.push(Header::To(to_header));
            }
            if let Some(cid) = server_tx.request.headers.call_id() {
                res.headers.push(Header::CallId(cid.to_string()));
            }
            if let Some(cseq) = server_tx.request.headers.cseq() {
                res.headers.push(Header::CSeq(cseq.clone()));
            }

            let local_contact = Uri::Sip(
                SipUri::new(self.transport.local_addr().ip().to_string())
                    .with_port(self.transport.local_addr().port()),
            );
            res.headers
                .push(Header::Contact(NameAddr::new(local_contact)));
            res.headers
                .push(Header::ContentType("application/sdp".to_string()));
            res.headers.push(Header::Generic(
                "User-Agent".to_string(),
                self.config.user_agent.clone(),
            ));
            res.body = sdp_str.into_bytes();
            res.headers.push(Header::ContentLength(res.body.len()));

            // Send 200 OK response
            server_tx
                .respond(res)
                .await
                .map_err(|e| PjsuaError::Media(e.to_string()))?;

            // Create UAS Dialog
            if let Ok(dialog) = Dialog::create_uas(&server_tx.request, local_tag) {
                self.dialogs.insert(dialog.clone()).await;
                call.set_dialog(dialog).await;
            }

            call.set_local_sdp(sdp_ans).await;
            call.set_media_session(media_session).await;
            call.set_state(CallState::Connecting).await;

            if let Some(r_addr) = remote_rtp {
                let _ = self.event_tx.send(PjsuaEvent::MediaStreamReady {
                    call_id: call_id.to_string(),
                    local_addr: rtp_bind_addr,
                    remote_addr: r_addr,
                    codec: "PCMU/8000".to_string(),
                });
            }

            let _ = self.event_tx.send(PjsuaEvent::CallStateChanged {
                call_id: call_id.to_string(),
                state: CallState::Connecting,
                status_code: 200,
                reason: "OK".to_string(),
            });
        } else {
            // Reject call e.g. 486 Busy Here
            let mut res = Response::new(status_code, "Busy Here");
            if let Some(via) = server_tx.request.headers.first_via() {
                res.headers.push(Header::Via(via.clone()));
            }
            if let Some(from) = server_tx.request.headers.from() {
                res.headers.push(Header::From(from.clone()));
            }
            if let Some(to) = server_tx.request.headers.to() {
                res.headers.push(Header::To(to.clone()));
            }
            if let Some(cid) = server_tx.request.headers.call_id() {
                res.headers.push(Header::CallId(cid.to_string()));
            }
            if let Some(cseq) = server_tx.request.headers.cseq() {
                res.headers.push(Header::CSeq(cseq.clone()));
            }
            res.headers.push(Header::ContentLength(0));
            let _ = server_tx.respond(res).await;

            call.set_state(CallState::Disconnected).await;
            let _ = self.event_tx.send(PjsuaEvent::CallStateChanged {
                call_id: call_id.to_string(),
                state: CallState::Disconnected,
                status_code,
                reason: "Declined".to_string(),
            });
        }

        Ok(())
    }

    pub async fn hangup_call(&self, call_id: &str) -> Result<()> {
        let call = self
            .get_call(call_id)
            .await
            .ok_or_else(|| PjsuaError::CallNotFound(call_id.to_string()))?;

        let state = call.state().await;
        if state == CallState::Confirmed {
            if let Some(mut dialog) = call.get_dialog().await {
                let bye = dialog.create_bye();
                let target: SocketAddr = match &dialog.remote_target {
                    Uri::Sip(s) => format!("{}:{}", s.host, s.port.unwrap_or(5060))
                        .parse()
                        .unwrap_or(self.transport.local_addr()),
                    _ => self.transport.local_addr(),
                };
                let _ = self.transactions.send_request(bye, target).await;
                self.dialogs.remove(&dialog.id).await;
            }
        } else if state == CallState::Incoming || state == CallState::Early {
            let _ = self.answer_call(call_id, 486).await;
        }

        call.set_state(CallState::Disconnected).await;
        let _ = self.event_tx.send(PjsuaEvent::CallStateChanged {
            call_id: call_id.to_string(),
            state: CallState::Disconnected,
            status_code: 200,
            reason: "Terminated".to_string(),
        });

        self.calls.lock().await.remove(call_id);
        self.server_txs.lock().await.remove(call_id);
        Ok(())
    }

    async fn handle_incoming_server_tx(&self, server_tx: Arc<ServerTransaction>) {
        let req = &server_tx.request;
        match req.method {
            Method::Invite => {
                let call_id = match req.headers.call_id() {
                    Some(cid) => cid.to_string(),
                    None => return,
                };
                let from_str = req
                    .headers
                    .from()
                    .map(|f| f.to_string())
                    .unwrap_or_default();
                let to_str = req.headers.to().map(|t| t.to_string()).unwrap_or_default();

                let call = Arc::new(Call::new(
                    call_id.clone(),
                    CallDirection::Incoming,
                    to_str.clone(),
                    from_str.clone(),
                    CallState::Incoming,
                ));

                // Parse remote SDP if attached
                if !req.body.is_empty()
                    && let Ok(body_str) = std::str::from_utf8(&req.body)
                    && let Ok(sdp) = SessionDescription::from_str(body_str)
                {
                    call.set_remote_sdp(sdp).await;
                }

                // Send 180 Ringing automatically
                let mut ring_res = Response::new(180, "Ringing");
                if let Some(via) = req.headers.first_via() {
                    ring_res.headers.push(Header::Via(via.clone()));
                }
                if let Some(from) = req.headers.from() {
                    ring_res.headers.push(Header::From(from.clone()));
                }
                if let Some(to) = req.headers.to() {
                    let local_tag = format!("tag-{}", rand_u32());
                    let mut to_h = to.clone();
                    to_h.parameters.push(("tag".to_string(), Some(local_tag)));
                    ring_res.headers.push(Header::To(to_h));
                }
                if let Some(cid) = req.headers.call_id() {
                    ring_res.headers.push(Header::CallId(cid.to_string()));
                }
                if let Some(cseq) = req.headers.cseq() {
                    ring_res.headers.push(Header::CSeq(cseq.clone()));
                }
                ring_res.headers.push(Header::ContentLength(0));

                let _ = server_tx.respond(ring_res).await;
                call.set_state(CallState::Early).await;

                self.calls
                    .lock()
                    .await
                    .insert(call_id.clone(), Arc::clone(&call));
                self.server_txs
                    .lock()
                    .await
                    .insert(call_id.clone(), Arc::clone(&server_tx));

                let _ = self.event_tx.send(PjsuaEvent::IncomingCall {
                    call_id: call_id.clone(),
                    from_uri: from_str,
                    to_uri: to_str,
                });
                let _ = self.event_tx.send(PjsuaEvent::CallStateChanged {
                    call_id,
                    state: CallState::Early,
                    status_code: 180,
                    reason: "Ringing".to_string(),
                });
            }
            Method::Bye => {
                let call_id = req.headers.call_id().map(|c| c.to_string());
                let mut res = Response::new(200, "OK");
                if let Some(via) = req.headers.first_via() {
                    res.headers.push(Header::Via(via.clone()));
                }
                if let Some(from) = req.headers.from() {
                    res.headers.push(Header::From(from.clone()));
                }
                if let Some(to) = req.headers.to() {
                    res.headers.push(Header::To(to.clone()));
                }
                if let Some(cid) = req.headers.call_id() {
                    res.headers.push(Header::CallId(cid.to_string()));
                }
                if let Some(cseq) = req.headers.cseq() {
                    res.headers.push(Header::CSeq(cseq.clone()));
                }
                res.headers.push(Header::ContentLength(0));
                let _ = server_tx.respond(res).await;

                if let Some(cid) = call_id
                    && let Some(call) = self.calls.lock().await.remove(&cid)
                {
                    call.set_state(CallState::Disconnected).await;
                    let _ = self.event_tx.send(PjsuaEvent::CallStateChanged {
                        call_id: cid,
                        state: CallState::Disconnected,
                        status_code: 200,
                        reason: "Terminated by BYE".to_string(),
                    });
                }
            }
            Method::Options => {
                let mut res = Response::new(200, "OK");
                if let Some(via) = req.headers.first_via() {
                    res.headers.push(Header::Via(via.clone()));
                }
                if let Some(from) = req.headers.from() {
                    res.headers.push(Header::From(from.clone()));
                }
                if let Some(to) = req.headers.to() {
                    res.headers.push(Header::To(to.clone()));
                }
                if let Some(cid) = req.headers.call_id() {
                    res.headers.push(Header::CallId(cid.to_string()));
                }
                if let Some(cseq) = req.headers.cseq() {
                    res.headers.push(Header::CSeq(cseq.clone()));
                }
                res.headers.push(Header::Allow(vec![
                    Method::Invite,
                    Method::Ack,
                    Method::Cancel,
                    Method::Bye,
                    Method::Options,
                ]));
                res.headers.push(Header::ContentLength(0));
                let _ = server_tx.respond(res).await;
            }
            _ => {
                let mut res = Response::new(501, "Not Implemented");
                if let Some(via) = req.headers.first_via() {
                    res.headers.push(Header::Via(via.clone()));
                }
                if let Some(from) = req.headers.from() {
                    res.headers.push(Header::From(from.clone()));
                }
                if let Some(to) = req.headers.to() {
                    res.headers.push(Header::To(to.clone()));
                }
                if let Some(cid) = req.headers.call_id() {
                    res.headers.push(Header::CallId(cid.to_string()));
                }
                if let Some(cseq) = req.headers.cseq() {
                    res.headers.push(Header::CSeq(cseq.clone()));
                }
                res.headers.push(Header::ContentLength(0));
                let _ = server_tx.respond(res).await;
            }
        }
    }

    async fn handle_incoming_ack(&self, ack_req: Request) {
        if let Some(cid) = ack_req.headers.call_id()
            && let Some(call) = self.get_call(cid).await
        {
            trace!("Received 2xx ACK for call {}", cid);
            call.set_state(CallState::Confirmed).await;
            let _ = self.event_tx.send(PjsuaEvent::CallStateChanged {
                call_id: cid.to_string(),
                state: CallState::Confirmed,
                status_code: 200,
                reason: "ACK received".to_string(),
            });
        }
    }
}

fn rand_u32() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    nanos ^ 0x5a5a_3c3c
}

fn rand_u64() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}
