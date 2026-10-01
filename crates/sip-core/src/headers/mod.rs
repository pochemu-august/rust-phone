pub mod auth;
pub mod via;

pub use auth::AuthHeader;
pub use via::ViaHeader;

use crate::error::{ParseError, Result};
use crate::method::Method;
use crate::uri::{NameAddr, parse_name_addr};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HeaderName {
    Via,
    From,
    To,
    CallId,
    CSeq,
    Contact,
    ContentLength,
    ContentType,
    MaxForwards,
    Expires,
    Allow,
    Supported,
    Require,
    Route,
    RecordRoute,
    Authorization,
    ProxyAuthorization,
    WwwAuthenticate,
    ProxyAuthenticate,
    UserAgent,
    Server,
    Subject,
    Custom(String),
}

impl HeaderName {
    pub fn as_str(&self) -> &str {
        match self {
            HeaderName::Via => "Via",
            HeaderName::From => "From",
            HeaderName::To => "To",
            HeaderName::CallId => "Call-ID",
            HeaderName::CSeq => "CSeq",
            HeaderName::Contact => "Contact",
            HeaderName::ContentLength => "Content-Length",
            HeaderName::ContentType => "Content-Type",
            HeaderName::MaxForwards => "Max-Forwards",
            HeaderName::Expires => "Expires",
            HeaderName::Allow => "Allow",
            HeaderName::Supported => "Supported",
            HeaderName::Require => "Require",
            HeaderName::Route => "Route",
            HeaderName::RecordRoute => "Record-Route",
            HeaderName::Authorization => "Authorization",
            HeaderName::ProxyAuthorization => "Proxy-Authorization",
            HeaderName::WwwAuthenticate => "WWW-Authenticate",
            HeaderName::ProxyAuthenticate => "Proxy-Authenticate",
            HeaderName::UserAgent => "User-Agent",
            HeaderName::Server => "Server",
            HeaderName::Subject => "Subject",
            HeaderName::Custom(s) => s.as_str(),
        }
    }

    pub fn from_raw(s: &str) -> Self {
        let trimmed = s.trim();
        if trimmed.eq_ignore_ascii_case("Via") || trimmed.eq_ignore_ascii_case("v") {
            HeaderName::Via
        } else if trimmed.eq_ignore_ascii_case("From") || trimmed.eq_ignore_ascii_case("f") {
            HeaderName::From
        } else if trimmed.eq_ignore_ascii_case("To") || trimmed.eq_ignore_ascii_case("t") {
            HeaderName::To
        } else if trimmed.eq_ignore_ascii_case("Call-ID") || trimmed.eq_ignore_ascii_case("i") {
            HeaderName::CallId
        } else if trimmed.eq_ignore_ascii_case("CSeq") {
            HeaderName::CSeq
        } else if trimmed.eq_ignore_ascii_case("Contact") || trimmed.eq_ignore_ascii_case("m") {
            HeaderName::Contact
        } else if trimmed.eq_ignore_ascii_case("Content-Length")
            || trimmed.eq_ignore_ascii_case("l")
        {
            HeaderName::ContentLength
        } else if trimmed.eq_ignore_ascii_case("Content-Type") || trimmed.eq_ignore_ascii_case("c")
        {
            HeaderName::ContentType
        } else if trimmed.eq_ignore_ascii_case("Max-Forwards") {
            HeaderName::MaxForwards
        } else if trimmed.eq_ignore_ascii_case("Expires") {
            HeaderName::Expires
        } else if trimmed.eq_ignore_ascii_case("Allow") || trimmed.eq_ignore_ascii_case("u") {
            HeaderName::Allow
        } else if trimmed.eq_ignore_ascii_case("Supported") || trimmed.eq_ignore_ascii_case("k") {
            HeaderName::Supported
        } else if trimmed.eq_ignore_ascii_case("Require") {
            HeaderName::Require
        } else if trimmed.eq_ignore_ascii_case("Route") {
            HeaderName::Route
        } else if trimmed.eq_ignore_ascii_case("Record-Route") {
            HeaderName::RecordRoute
        } else if trimmed.eq_ignore_ascii_case("Authorization") {
            HeaderName::Authorization
        } else if trimmed.eq_ignore_ascii_case("Proxy-Authorization") {
            HeaderName::ProxyAuthorization
        } else if trimmed.eq_ignore_ascii_case("WWW-Authenticate") {
            HeaderName::WwwAuthenticate
        } else if trimmed.eq_ignore_ascii_case("Proxy-Authenticate") {
            HeaderName::ProxyAuthenticate
        } else if trimmed.eq_ignore_ascii_case("User-Agent") {
            HeaderName::UserAgent
        } else if trimmed.eq_ignore_ascii_case("Server") {
            HeaderName::Server
        } else if trimmed.eq_ignore_ascii_case("Subject") || trimmed.eq_ignore_ascii_case("s") {
            HeaderName::Subject
        } else {
            HeaderName::Custom(trimmed.to_string())
        }
    }
}

impl fmt::Display for HeaderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CSeqHeader {
    pub seq: u32,
    pub method: Method,
}

impl CSeqHeader {
    pub fn new(seq: u32, method: Method) -> Self {
        Self { seq, method }
    }
}

impl fmt::Display for CSeqHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.seq, self.method)
    }
}

impl FromStr for CSeqHeader {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self> {
        let trimmed = s.trim();
        let (seq_str, method_str) = trimmed.split_once(' ').ok_or_else(|| {
            ParseError::InvalidCSeq(format!("Expected sequence and method: {}", trimmed))
        })?;

        let seq = seq_str.trim().parse::<u32>().map_err(|_| {
            ParseError::InvalidCSeq(format!("Invalid integer sequence in CSeq: {}", seq_str))
        })?;
        let method = Method::from_str(method_str.trim())?;

        Ok(CSeqHeader { seq, method })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Header {
    Via(ViaHeader),
    From(NameAddr),
    To(NameAddr),
    CallId(String),
    CSeq(CSeqHeader),
    Contact(NameAddr),
    ContentLength(usize),
    ContentType(String),
    MaxForwards(u32),
    Expires(u32),
    Allow(Vec<Method>),
    Supported(Vec<String>),
    Route(NameAddr),
    RecordRoute(NameAddr),
    Authorization(AuthHeader),
    ProxyAuthorization(AuthHeader),
    WwwAuthenticate(AuthHeader),
    ProxyAuthenticate(AuthHeader),
    Generic(String, String),
}

impl Header {
    pub fn name(&self) -> HeaderName {
        match self {
            Header::Via(_) => HeaderName::Via,
            Header::From(_) => HeaderName::From,
            Header::To(_) => HeaderName::To,
            Header::CallId(_) => HeaderName::CallId,
            Header::CSeq(_) => HeaderName::CSeq,
            Header::Contact(_) => HeaderName::Contact,
            Header::ContentLength(_) => HeaderName::ContentLength,
            Header::ContentType(_) => HeaderName::ContentType,
            Header::MaxForwards(_) => HeaderName::MaxForwards,
            Header::Expires(_) => HeaderName::Expires,
            Header::Allow(_) => HeaderName::Allow,
            Header::Supported(_) => HeaderName::Supported,
            Header::Route(_) => HeaderName::Route,
            Header::RecordRoute(_) => HeaderName::RecordRoute,
            Header::Authorization(_) => HeaderName::Authorization,
            Header::ProxyAuthorization(_) => HeaderName::ProxyAuthorization,
            Header::WwwAuthenticate(_) => HeaderName::WwwAuthenticate,
            Header::ProxyAuthenticate(_) => HeaderName::ProxyAuthenticate,
            Header::Generic(name, _) => HeaderName::from_raw(name),
        }
    }

    pub fn value_string(&self) -> String {
        match self {
            Header::Via(v) => v.to_string(),
            Header::From(f) => f.to_string(),
            Header::To(t) => t.to_string(),
            Header::CallId(c) => c.clone(),
            Header::CSeq(cs) => cs.to_string(),
            Header::Contact(ct) => ct.to_string(),
            Header::ContentLength(cl) => cl.to_string(),
            Header::ContentType(ct) => ct.clone(),
            Header::MaxForwards(mf) => mf.to_string(),
            Header::Expires(exp) => exp.to_string(),
            Header::Allow(methods) => methods
                .iter()
                .map(|m| m.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            Header::Supported(s) => s.join(", "),
            Header::Route(r) => r.to_string(),
            Header::RecordRoute(rr) => rr.to_string(),
            Header::Authorization(a) => a.to_string(),
            Header::ProxyAuthorization(a) => a.to_string(),
            Header::WwwAuthenticate(a) => a.to_string(),
            Header::ProxyAuthenticate(a) => a.to_string(),
            Header::Generic(_, val) => val.clone(),
        }
    }

    pub fn parse(name_str: &str, val_str: &str) -> Result<Vec<Header>> {
        let name = HeaderName::from_raw(name_str);
        let val = val_str.trim();

        match name {
            HeaderName::Via => {
                // Via can be comma-separated list of via headers
                let mut vias = Vec::new();
                for item in split_comma_separated(val) {
                    vias.push(Header::Via(ViaHeader::from_str(&item)?));
                }
                Ok(vias)
            }
            HeaderName::From => Ok(vec![Header::From(parse_name_addr(val)?)]),
            HeaderName::To => Ok(vec![Header::To(parse_name_addr(val)?)]),
            HeaderName::CallId => Ok(vec![Header::CallId(val.to_string())]),
            HeaderName::CSeq => Ok(vec![Header::CSeq(CSeqHeader::from_str(val)?)]),
            HeaderName::Contact => {
                let mut contacts = Vec::new();
                for item in split_comma_separated(val) {
                    contacts.push(Header::Contact(parse_name_addr(&item)?));
                }
                Ok(contacts)
            }
            HeaderName::ContentLength => {
                let cl = val.parse::<usize>().map_err(|_| {
                    ParseError::InvalidContentLength(format!(
                        "Invalid integer in Content-Length: {}",
                        val
                    ))
                })?;
                Ok(vec![Header::ContentLength(cl)])
            }
            HeaderName::ContentType => Ok(vec![Header::ContentType(val.to_string())]),
            HeaderName::MaxForwards => {
                let mf = val.parse::<u32>().map_err(|_| {
                    ParseError::InvalidHeader(format!("Invalid Max-Forwards: {}", val))
                })?;
                Ok(vec![Header::MaxForwards(mf)])
            }
            HeaderName::Expires => {
                let exp = val
                    .parse::<u32>()
                    .map_err(|_| ParseError::InvalidHeader(format!("Invalid Expires: {}", val)))?;
                Ok(vec![Header::Expires(exp)])
            }
            HeaderName::Allow => {
                let mut methods = Vec::new();
                for m in split_comma_separated(val) {
                    methods.push(Method::from_str(&m)?);
                }
                Ok(vec![Header::Allow(methods)])
            }
            HeaderName::Supported => {
                let items = split_comma_separated(val);
                Ok(vec![Header::Supported(items)])
            }
            HeaderName::Route => {
                let mut routes = Vec::new();
                for item in split_comma_separated(val) {
                    routes.push(Header::Route(parse_name_addr(&item)?));
                }
                Ok(routes)
            }
            HeaderName::RecordRoute => {
                let mut rroutes = Vec::new();
                for item in split_comma_separated(val) {
                    rroutes.push(Header::RecordRoute(parse_name_addr(&item)?));
                }
                Ok(rroutes)
            }
            HeaderName::Authorization => {
                Ok(vec![Header::Authorization(AuthHeader::from_str(val)?)])
            }
            HeaderName::ProxyAuthorization => {
                Ok(vec![Header::ProxyAuthorization(AuthHeader::from_str(val)?)])
            }
            HeaderName::WwwAuthenticate => {
                Ok(vec![Header::WwwAuthenticate(AuthHeader::from_str(val)?)])
            }
            HeaderName::ProxyAuthenticate => {
                Ok(vec![Header::ProxyAuthenticate(AuthHeader::from_str(val)?)])
            }
            _ => Ok(vec![Header::Generic(
                name_str.trim().to_string(),
                val.to_string(),
            )]),
        }
    }
}

impl fmt::Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name(), self.value_string())
    }
}

/// Helper to split comma-separated values while ignoring commas inside quotes and `<>`
fn split_comma_separated(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut in_angle = false;
    let mut escape = false;

    for c in s.chars() {
        if escape {
            current.push(c);
            escape = false;
            continue;
        }

        match c {
            '\\' => {
                escape = true;
                current.push(c);
            }
            '"' => {
                in_quote = !in_quote;
                current.push(c);
            }
            '<' if !in_quote => {
                in_angle = true;
                current.push(c);
            }
            '>' if !in_quote => {
                in_angle = false;
                current.push(c);
            }
            ',' if !in_quote && !in_angle => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    result.push(trimmed.to_string());
                }
                current.clear();
            }
            _ => {
                current.push(c);
            }
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        result.push(trimmed.to_string());
    }

    result
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Headers {
    headers: Vec<Header>,
}

impl Headers {
    pub fn new() -> Self {
        Self {
            headers: Vec::new(),
        }
    }

    pub fn push(&mut self, header: Header) {
        self.headers.push(header);
    }

    pub fn insert(&mut self, index: usize, header: Header) {
        self.headers.insert(index, header);
    }

    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&Header) -> bool,
    {
        self.headers.retain(f);
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Header> {
        self.headers.iter()
    }

    pub fn vias(&self) -> Vec<&ViaHeader> {
        self.headers
            .iter()
            .filter_map(|h| match h {
                Header::Via(v) => Some(v),
                _ => None,
            })
            .collect()
    }

    pub fn first_via(&self) -> Option<&ViaHeader> {
        self.vias().into_iter().next()
    }

    pub fn from(&self) -> Option<&NameAddr> {
        self.headers.iter().find_map(|h| match h {
            Header::From(f) => Some(f),
            _ => None,
        })
    }

    pub fn to(&self) -> Option<&NameAddr> {
        self.headers.iter().find_map(|h| match h {
            Header::To(t) => Some(t),
            _ => None,
        })
    }

    pub fn set_to(&mut self, to: NameAddr) {
        if let Some(pos) = self.headers.iter().position(|h| matches!(h, Header::To(_))) {
            self.headers[pos] = Header::To(to);
        } else {
            self.headers.push(Header::To(to));
        }
    }

    pub fn set_to_tag(&mut self, tag: impl Into<String>) {
        if let Some(Header::To(to)) = self.headers.iter_mut().find(|h| matches!(h, Header::To(_))) {
            to.parameters
                .retain(|(k, _)| !k.eq_ignore_ascii_case("tag"));
            to.parameters.push(("tag".to_string(), Some(tag.into())));
        }
    }

    pub fn call_id(&self) -> Option<&str> {
        self.headers.iter().find_map(|h| match h {
            Header::CallId(c) => Some(c.as_str()),
            _ => None,
        })
    }

    pub fn cseq(&self) -> Option<&CSeqHeader> {
        self.headers.iter().find_map(|h| match h {
            Header::CSeq(cs) => Some(cs),
            _ => None,
        })
    }

    pub fn contacts(&self) -> Vec<&NameAddr> {
        self.headers
            .iter()
            .filter_map(|h| match h {
                Header::Contact(c) => Some(c),
                _ => None,
            })
            .collect()
    }

    pub fn first_contact(&self) -> Option<&NameAddr> {
        self.contacts().into_iter().next()
    }

    pub fn content_length(&self) -> Option<usize> {
        self.headers.iter().find_map(|h| match h {
            Header::ContentLength(cl) => Some(*cl),
            _ => None,
        })
    }

    pub fn content_type(&self) -> Option<&str> {
        self.headers.iter().find_map(|h| match h {
            Header::ContentType(ct) => Some(ct.as_str()),
            _ => None,
        })
    }

    pub fn max_forwards(&self) -> Option<u32> {
        self.headers.iter().find_map(|h| match h {
            Header::MaxForwards(mf) => Some(*mf),
            _ => None,
        })
    }

    pub fn expires(&self) -> Option<u32> {
        self.headers.iter().find_map(|h| match h {
            Header::Expires(exp) => Some(*exp),
            _ => None,
        })
    }

    pub fn get_generic(&self, name: &str) -> Option<&str> {
        self.headers.iter().find_map(|h| match h {
            Header::Generic(n, v) if n.eq_ignore_ascii_case(name) => Some(v.as_str()),
            _ => None,
        })
    }
}

impl IntoIterator for Headers {
    type Item = Header;
    type IntoIter = std::vec::IntoIter<Header>;

    fn into_iter(self) -> Self::IntoIter {
        self.headers.into_iter()
    }
}
