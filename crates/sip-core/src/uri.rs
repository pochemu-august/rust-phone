use crate::error::{ParseError, Result};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Scheme {
    Sip,
    Sips,
    Other(String),
}

impl Scheme {
    pub fn as_str(&self) -> &str {
        match self {
            Scheme::Sip => "sip",
            Scheme::Sips => "sips",
            Scheme::Other(s) => s.as_str(),
        }
    }
}

impl fmt::Display for Scheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SipUri {
    pub scheme: Scheme,
    pub user: Option<String>,
    pub password: Option<String>,
    pub host: String,
    pub port: Option<u16>,
    pub parameters: Vec<(String, Option<String>)>,
    pub headers: Vec<(String, String)>,
}

impl SipUri {
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            scheme: Scheme::Sip,
            user: None,
            password: None,
            host: host.into(),
            port: None,
            parameters: Vec::new(),
            headers: Vec::new(),
        }
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn with_param(mut self, key: impl Into<String>, val: Option<impl Into<String>>) -> Self {
        self.parameters.push((key.into(), val.map(|v| v.into())));
        self
    }

    pub fn get_param(&self, key: &str) -> Option<Option<&str>> {
        self.parameters
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_deref())
    }

    pub fn has_param(&self, key: &str) -> bool {
        self.parameters
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case(key))
    }

    pub fn is_lr(&self) -> bool {
        self.has_param("lr")
    }

    pub fn transport(&self) -> Option<&str> {
        self.get_param("transport").flatten()
    }
}

impl fmt::Display for SipUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:", self.scheme)?;
        if let Some(user) = &self.user {
            write!(f, "{}", user)?;
            if let Some(pass) = &self.password {
                write!(f, ":{}", pass)?;
            }
            write!(f, "@")?;
        }
        write!(f, "{}", self.host)?;
        if let Some(port) = self.port {
            write!(f, ":{}", port)?;
        }
        for (k, v) in &self.parameters {
            write!(f, ";{}", k)?;
            if let Some(val) = v {
                if val.contains(' ') || val.contains(';') || val.contains('"') {
                    write!(f, "=\"{}\"", val.replace('"', "\\\""))?;
                } else {
                    write!(f, "={}", val)?;
                }
            }
        }
        if !self.headers.is_empty() {
            write!(f, "?")?;
            for (idx, (k, v)) in self.headers.iter().enumerate() {
                if idx > 0 {
                    write!(f, "&")?;
                }
                write!(f, "{}={}", k, v)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Uri {
    Sip(SipUri),
    Other(String),
}

impl Uri {
    pub fn is_sip(&self) -> bool {
        matches!(self, Uri::Sip(_))
    }

    pub fn sip(&self) -> Option<&SipUri> {
        match self {
            Uri::Sip(uri) => Some(uri),
            _ => None,
        }
    }

    pub fn sip_mut(&mut self) -> Option<&mut SipUri> {
        match self {
            Uri::Sip(uri) => Some(uri),
            _ => None,
        }
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Uri::Sip(s) => write!(f, "{}", s),
            Uri::Other(o) => write!(f, "{}", o),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameAddr {
    pub display_name: Option<String>,
    pub uri: Uri,
    pub parameters: Vec<(String, Option<String>)>,
}

impl NameAddr {
    pub fn new(uri: Uri) -> Self {
        Self {
            display_name: None,
            uri,
            parameters: Vec::new(),
        }
    }

    pub fn with_display(mut self, display: impl Into<String>) -> Self {
        self.display_name = Some(display.into());
        self
    }

    pub fn with_param(mut self, key: impl Into<String>, val: Option<impl Into<String>>) -> Self {
        self.parameters.push((key.into(), val.map(|v| v.into())));
        self
    }

    pub fn get_param(&self, key: &str) -> Option<Option<&str>> {
        self.parameters
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_deref())
    }

    pub fn tag(&self) -> Option<&str> {
        self.get_param("tag").flatten()
    }
}

impl fmt::Display for NameAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(display) = &self.display_name {
            if display.contains('"')
                || display.contains(' ')
                || display.contains(',')
                || display.contains(';')
            {
                write!(f, "\"{}\" ", display.replace('"', "\\\""))?;
            } else {
                write!(f, "{} ", display)?;
            }
            write!(f, "<{}>", self.uri)?;
        } else if matches!(&self.uri, Uri::Other(s) if s == "*") {
            write!(f, "*")?;
        } else {
            write!(f, "<{}>", self.uri)?;
        }

        for (k, v) in &self.parameters {
            write!(f, ";{}", k)?;
            if let Some(val) = v {
                if val.contains(' ') || val.contains(';') || val.contains('"') {
                    write!(f, "=\"{}\"", val.replace('"', "\\\""))?;
                } else {
                    write!(f, "={}", val)?;
                }
            }
        }
        Ok(())
    }
}

impl FromStr for SipUri {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self> {
        parse_sip_uri(s)
    }
}

impl FromStr for Uri {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self> {
        parse_uri(s)
    }
}

impl FromStr for NameAddr {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self> {
        parse_name_addr(s)
    }
}

/// Parses a SIP or SIPS URI string
pub fn parse_sip_uri(input: &str) -> Result<SipUri> {
    let s = input.trim();
    let (scheme, rest) = if let Some(rest) = s.strip_prefix("sip:") {
        (Scheme::Sip, rest)
    } else if let Some(rest) = s.strip_prefix("sips:") {
        (Scheme::Sips, rest)
    } else {
        return Err(ParseError::InvalidUri(format!(
            "Missing sip: or sips: scheme in '{}'",
            s
        )));
    };

    // Separate headers if any (prefixed by '?')
    let (main_part, headers_part) = match rest.find('?') {
        Some(idx) => (&rest[..idx], Some(&rest[idx + 1..])),
        None => (rest, None),
    };

    // Separate URI parameters (prefixed by ';')
    let mut param_splits = main_part.split(';');
    let authority = param_splits.next().unwrap_or("");

    let mut parameters = Vec::new();
    for p in param_splits {
        if p.is_empty() {
            continue;
        }
        if let Some((k, v)) = p.split_once('=') {
            let clean_v = if v.starts_with('"') && v.ends_with('"') && v.len() >= 2 {
                &v[1..v.len() - 1]
            } else {
                v
            };
            parameters.push((k.trim().to_string(), Some(clean_v.trim().to_string())));
        } else {
            parameters.push((p.trim().to_string(), None));
        }
    }

    let mut headers = Vec::new();
    if let Some(h_part) = headers_part {
        for h in h_part.split('&') {
            if h.is_empty() {
                continue;
            }
            if let Some((k, v)) = h.split_once('=') {
                headers.push((k.to_string(), v.to_string()));
            } else {
                headers.push((h.to_string(), String::new()));
            }
        }
    }

    // Parse userinfo @ host:port
    let (user_info, host_port) = match authority.rfind('@') {
        Some(idx) => (Some(&authority[..idx]), &authority[idx + 1..]),
        None => (None, authority),
    };

    let (user, password) = if let Some(ui) = user_info {
        match ui.split_once(':') {
            Some((u, p)) => (Some(u.to_string()), Some(p.to_string())),
            None => (Some(ui.to_string()), None),
        }
    } else {
        (None, None)
    };

    let (host, port) = parse_host_port(host_port)?;

    Ok(SipUri {
        scheme,
        user,
        password,
        host,
        port,
        parameters,
        headers,
    })
}

pub fn parse_host_port(hp: &str) -> Result<(String, Option<u16>)> {
    let hp = hp.trim();
    if hp.is_empty() {
        return Err(ParseError::InvalidUri("Empty host in URI".into()));
    }

    if hp.starts_with('[') {
        // IPv6
        if let Some(close_bracket) = hp.find(']') {
            let host = hp[..=close_bracket].to_string();
            let remainder = &hp[close_bracket + 1..];
            let port = if let Some(p) = remainder.strip_prefix(':') {
                Some(
                    p.parse::<u16>()
                        .map_err(|_| ParseError::InvalidUri(format!("Invalid port: {}", p)))?,
                )
            } else {
                None
            };
            return Ok((host, port));
        } else {
            return Err(ParseError::InvalidUri(
                "Unclosed IPv6 bracket in host".into(),
            ));
        }
    }

    match hp.split_once(':') {
        Some((h, p)) => {
            let port = p
                .parse::<u16>()
                .map_err(|_| ParseError::InvalidUri(format!("Invalid port: {}", p)))?;
            Ok((h.to_string(), Some(port)))
        }
        None => Ok((hp.to_string(), None)),
    }
}

pub fn parse_uri(input: &str) -> Result<Uri> {
    let s = input.trim();
    if s.starts_with("sip:") || s.starts_with("sips:") {
        Ok(Uri::Sip(parse_sip_uri(s)?))
    } else {
        Ok(Uri::Other(s.to_string()))
    }
}

pub fn parse_name_addr(input: &str) -> Result<NameAddr> {
    let s = input.trim();
    if s == "*" {
        return Ok(NameAddr::new(Uri::Other("*".into())));
    }

    let mut display_name = None;

    // Check if there is `<...>`
    if let Some(start_angle) = s.find('<') {
        let before_angle = s[..start_angle].trim();
        if !before_angle.is_empty() {
            let d = if before_angle.starts_with('"')
                && before_angle.ends_with('"')
                && before_angle.len() >= 2
            {
                &before_angle[1..before_angle.len() - 1]
            } else {
                before_angle
            };
            display_name = Some(d.replace("\\\"", "\""));
        }

        let after_start = &s[start_angle + 1..];
        let end_angle = after_start
            .find('>')
            .ok_or_else(|| ParseError::InvalidUri("Missing closing '>' in NameAddr".into()))?;

        let uri_str = &after_start[..end_angle];
        let uri = parse_uri(uri_str)?;

        let remainder = &after_start[end_angle + 1..].trim();
        let mut parameters = Vec::new();
        if !remainder.is_empty() {
            for p in remainder.split(';') {
                let p = p.trim();
                if p.is_empty() {
                    continue;
                }
                if let Some((k, v)) = p.split_once('=') {
                    let clean_v = if v.starts_with('"') && v.ends_with('"') && v.len() >= 2 {
                        &v[1..v.len() - 1]
                    } else {
                        v
                    };
                    parameters.push((k.trim().to_string(), Some(clean_v.trim().to_string())));
                } else {
                    parameters.push((p.to_string(), None));
                }
            }
        }

        Ok(NameAddr {
            display_name,
            uri,
            parameters,
        })
    } else {
        // No angle brackets.
        // It could be: sip:user@host;tag=123
        // Note: RFC 3261 says if params follow without angle brackets, params belong to URI unless specified in context.
        // But for From/To/Contact, if no angle brackets, semicolon after host marks URI params.
        let uri = parse_uri(s)?;
        Ok(NameAddr {
            display_name: None,
            uri,
            parameters: Vec::new(),
        })
    }
}
