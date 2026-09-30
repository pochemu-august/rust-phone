use crate::headers::{Header, Headers};
use crate::method::Method;
use crate::uri::Uri;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    pub uri: Uri,
    pub version: String,
    pub headers: Headers,
    pub body: Vec<u8>,
}

impl Request {
    pub fn new(method: Method, uri: Uri) -> Self {
        Self {
            method,
            uri,
            version: "SIP/2.0".to_string(),
            headers: Headers::new(),
            body: Vec::new(),
        }
    }

    pub fn with_header(mut self, header: Header) -> Self {
        self.headers.push(header);
        self
    }

    pub fn with_body(mut self, body: impl Into<Vec<u8>>, content_type: impl Into<String>) -> Self {
        let b = body.into();
        self.headers.push(Header::ContentType(content_type.into()));
        self.headers.push(Header::ContentLength(b.len()));
        self.body = b;
        self
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Request line
        let line = format!("{} {} {}\r\n", self.method, self.uri, self.version);
        buf.extend_from_slice(line.as_bytes());

        // Headers
        for h in self.headers.iter() {
            let h_line = format!("{}\r\n", h);
            buf.extend_from_slice(h_line.as_bytes());
        }

        // Empty line separator
        buf.extend_from_slice(b"\r\n");

        // Body
        if !self.body.is_empty() {
            buf.extend_from_slice(&self.body);
        }

        buf
    }
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}\r\n", self.method, self.uri, self.version)?;
        for h in self.headers.iter() {
            write!(f, "{}\r\n", h)?;
        }
        write!(f, "\r\n")?;
        if !self.body.is_empty() {
            write!(f, "{}", String::from_utf8_lossy(&self.body))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub version: String,
    pub status_code: u16,
    pub reason_phrase: String,
    pub headers: Headers,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status_code: u16, reason_phrase: impl Into<String>) -> Self {
        Self {
            version: "SIP/2.0".to_string(),
            status_code,
            reason_phrase: reason_phrase.into(),
            headers: Headers::new(),
            body: Vec::new(),
        }
    }

    pub fn is_provisional(&self) -> bool {
        self.status_code >= 100 && self.status_code < 200
    }

    pub fn is_success(&self) -> bool {
        self.status_code >= 200 && self.status_code < 300
    }

    pub fn is_redirection(&self) -> bool {
        self.status_code >= 300 && self.status_code < 400
    }

    pub fn is_client_error(&self) -> bool {
        self.status_code >= 400 && self.status_code < 500
    }

    pub fn is_server_error(&self) -> bool {
        self.status_code >= 500 && self.status_code < 600
    }

    pub fn is_global_failure(&self) -> bool {
        self.status_code >= 600 && self.status_code < 700
    }

    pub fn is_final(&self) -> bool {
        self.status_code >= 200
    }

    pub fn with_header(mut self, header: Header) -> Self {
        self.headers.push(header);
        self
    }

    pub fn with_body(mut self, body: impl Into<Vec<u8>>, content_type: impl Into<String>) -> Self {
        let b = body.into();
        self.headers.push(Header::ContentType(content_type.into()));
        self.headers.push(Header::ContentLength(b.len()));
        self.body = b;
        self
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Status line
        let line = format!(
            "{} {} {}\r\n",
            self.version, self.status_code, self.reason_phrase
        );
        buf.extend_from_slice(line.as_bytes());

        // Headers
        for h in self.headers.iter() {
            let h_line = format!("{}\r\n", h);
            buf.extend_from_slice(h_line.as_bytes());
        }

        // Empty line separator
        buf.extend_from_slice(b"\r\n");

        // Body
        if !self.body.is_empty() {
            buf.extend_from_slice(&self.body);
        }

        buf
    }

    /// Creates a response for a given request, copying Via, From, To, Call-ID, CSeq
    pub fn reply_to(req: &Request, status_code: u16, reason: impl Into<String>) -> Self {
        let mut res = Self::new(status_code, reason);
        for via in req.headers.vias() {
            res.headers.push(Header::Via((*via).clone()));
        }
        if let Some(from) = req.headers.from() {
            res.headers.push(Header::From(from.clone()));
        }
        if let Some(to) = req.headers.to() {
            res.headers.push(Header::To(to.clone()));
        }
        if let Some(call_id) = req.headers.call_id() {
            res.headers.push(Header::CallId(call_id.to_string()));
        }
        if let Some(cseq) = req.headers.cseq() {
            res.headers.push(Header::CSeq(cseq.clone()));
        }
        res.headers.push(Header::ContentLength(0));
        res
    }
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {}\r\n",
            self.version, self.status_code, self.reason_phrase
        )?;
        for h in self.headers.iter() {
            write!(f, "{}\r\n", h)?;
        }
        write!(f, "\r\n")?;
        if !self.body.is_empty() {
            write!(f, "{}", String::from_utf8_lossy(&self.body))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SipMessage {
    Request(Request),
    Response(Response),
}

impl SipMessage {
    pub fn is_request(&self) -> bool {
        matches!(self, SipMessage::Request(_))
    }

    pub fn is_response(&self) -> bool {
        matches!(self, SipMessage::Response(_))
    }

    pub fn request(&self) -> Option<&Request> {
        match self {
            SipMessage::Request(r) => Some(r),
            _ => None,
        }
    }

    pub fn response(&self) -> Option<&Response> {
        match self {
            SipMessage::Response(r) => Some(r),
            _ => None,
        }
    }

    pub fn headers(&self) -> &Headers {
        match self {
            SipMessage::Request(r) => &r.headers,
            SipMessage::Response(r) => &r.headers,
        }
    }

    pub fn body(&self) -> &[u8] {
        match self {
            SipMessage::Request(r) => &r.body,
            SipMessage::Response(r) => &r.body,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            SipMessage::Request(r) => r.to_bytes(),
            SipMessage::Response(r) => r.to_bytes(),
        }
    }
}

impl fmt::Display for SipMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SipMessage::Request(r) => write!(f, "{}", r),
            SipMessage::Response(r) => write!(f, "{}", r),
        }
    }
}
