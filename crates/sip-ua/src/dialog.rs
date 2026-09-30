use crate::error::{Result, UaError};
use sip_core::{CSeqHeader, Header, Method, NameAddr, Request, Response, Uri};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogState {
    Early,
    Confirmed,
    Terminated,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DialogId {
    pub call_id: String,
    pub local_tag: String,
    pub remote_tag: String,
}

impl DialogId {
    pub fn new(
        call_id: impl Into<String>,
        local_tag: impl Into<String>,
        remote_tag: impl Into<String>,
    ) -> Self {
        Self {
            call_id: call_id.into(),
            local_tag: local_tag.into(),
            remote_tag: remote_tag.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Dialog {
    pub id: DialogId,
    pub state: DialogState,
    pub local_cseq: u32,
    pub remote_cseq: Option<u32>,
    pub local_uri: NameAddr,
    pub remote_uri: NameAddr,
    pub remote_target: Uri,
    pub route_set: Vec<NameAddr>,
    pub secure: bool,
}

impl Dialog {
    /// Creates a UAC Dialog from an original INVITE request and a 1xx/2xx response
    pub fn create_uac(req: &Request, res: &Response) -> Result<Self> {
        let call_id = req
            .headers
            .call_id()
            .ok_or(UaError::MissingHeader("Call-ID"))?
            .to_string();

        let local_tag = req
            .headers
            .from()
            .and_then(|f| f.tag())
            .ok_or(UaError::MissingHeader("From tag"))?
            .to_string();

        let remote_tag = res
            .headers
            .to()
            .and_then(|t| t.tag())
            .ok_or(UaError::MissingHeader("To tag"))?
            .to_string();

        let local_cseq = req
            .headers
            .cseq()
            .ok_or(UaError::MissingHeader("CSeq"))?
            .seq;

        let local_uri = req
            .headers
            .from()
            .cloned()
            .ok_or(UaError::MissingHeader("From"))?;

        let mut remote_uri = req
            .headers
            .to()
            .cloned()
            .ok_or(UaError::MissingHeader("To"))?;
        // Update To with remote tag
        remote_uri
            .parameters
            .retain(|(k, _)| !k.eq_ignore_ascii_case("tag"));
        remote_uri
            .parameters
            .push(("tag".to_string(), Some(remote_tag.clone())));

        // Remote target from Contact header
        let remote_target = res
            .headers
            .first_contact()
            .map(|c| c.uri.clone())
            .ok_or(UaError::MissingHeader("Contact"))?;

        // Route set from Record-Route headers (for UAC, reverse order RFC 3261 12.2.1.1)
        let mut route_set: Vec<NameAddr> = res
            .headers
            .iter()
            .filter_map(|h| match h {
                Header::RecordRoute(rr) => Some(rr.clone()),
                _ => None,
            })
            .collect();
        route_set.reverse();

        let state = if res.status_code < 200 {
            DialogState::Early
        } else {
            DialogState::Confirmed
        };

        Ok(Self {
            id: DialogId::new(call_id, local_tag, remote_tag),
            state,
            local_cseq,
            remote_cseq: None,
            local_uri,
            remote_uri,
            remote_target,
            route_set,
            secure: false,
        })
    }

    /// Creates a UAS Dialog from an incoming INVITE request and local tag (RFC 3261 12.1.1)
    pub fn create_uas(req: &Request, local_tag: impl Into<String>) -> Result<Self> {
        let call_id = req
            .headers
            .call_id()
            .ok_or(UaError::MissingHeader("Call-ID"))?
            .to_string();

        let remote_tag = req
            .headers
            .from()
            .and_then(|f| f.tag())
            .ok_or(UaError::MissingHeader("From tag"))?
            .to_string();

        let local_tag = local_tag.into();

        let remote_cseq = req.headers.cseq().map(|c| c.seq);

        let mut local_uri = req
            .headers
            .to()
            .cloned()
            .ok_or(UaError::MissingHeader("To"))?;
        // Set local tag on To header
        local_uri
            .parameters
            .retain(|(k, _)| !k.eq_ignore_ascii_case("tag"));
        local_uri
            .parameters
            .push(("tag".to_string(), Some(local_tag.clone())));

        let remote_uri = req
            .headers
            .from()
            .cloned()
            .ok_or(UaError::MissingHeader("From"))?;

        let remote_target = req
            .headers
            .first_contact()
            .map(|c| c.uri.clone())
            .ok_or(UaError::MissingHeader("Contact"))?;

        // Route set from Record-Route headers (for UAS, keep original order RFC 3261 12.1.1)
        let route_set: Vec<NameAddr> = req
            .headers
            .iter()
            .filter_map(|h| match h {
                Header::RecordRoute(rr) => Some(rr.clone()),
                _ => None,
            })
            .collect();

        Ok(Self {
            id: DialogId::new(call_id, local_tag, remote_tag),
            state: DialogState::Confirmed,
            local_cseq: 0,
            remote_cseq,
            local_uri,
            remote_uri,
            remote_target,
            route_set,
            secure: false,
        })
    }

    /// Formats an in-dialog request (e.g. BYE, re-INVITE, INFO)
    pub fn create_request(&mut self, method: Method) -> Request {
        self.local_cseq += 1;

        let mut req = Request::new(method.clone(), self.remote_target.clone());

        req.headers.push(Header::From(self.local_uri.clone()));
        req.headers.push(Header::To(self.remote_uri.clone()));
        req.headers.push(Header::CallId(self.id.call_id.clone()));
        req.headers
            .push(Header::CSeq(CSeqHeader::new(self.local_cseq, method)));
        req.headers.push(Header::MaxForwards(70));

        // Add Route headers
        for r in &self.route_set {
            req.headers.push(Header::Route(r.clone()));
        }

        req.headers.push(Header::ContentLength(0));
        req
    }

    /// Formats an ACK for a 2xx response (RFC 3261 13.2.2.4)
    pub fn create_ack_for_2xx(&self, res_2xx: &Response) -> Result<Request> {
        let mut ack = Request::new(Method::Ack, self.remote_target.clone());

        ack.headers.push(Header::From(self.local_uri.clone()));
        ack.headers.push(Header::To(self.remote_uri.clone()));
        ack.headers.push(Header::CallId(self.id.call_id.clone()));

        let cseq_num = res_2xx
            .headers
            .cseq()
            .map(|c| c.seq)
            .unwrap_or(self.local_cseq);
        ack.headers
            .push(Header::CSeq(CSeqHeader::new(cseq_num, Method::Ack)));
        ack.headers.push(Header::MaxForwards(70));

        for r in &self.route_set {
            ack.headers.push(Header::Route(r.clone()));
        }

        ack.headers.push(Header::ContentLength(0));
        Ok(ack)
    }

    /// Creates a BYE request and sets dialog state to Terminated
    pub fn create_bye(&mut self) -> Request {
        self.state = DialogState::Terminated;
        self.create_request(Method::Bye)
    }
}
