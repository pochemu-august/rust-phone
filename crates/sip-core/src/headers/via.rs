use crate::error::{ParseError, Result};
use crate::uri::parse_host_port;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViaHeader {
    pub protocol: String, // e.g. "SIP/2.0/UDP"
    pub sent_by_host: String,
    pub sent_by_port: Option<u16>,
    pub parameters: Vec<(String, Option<String>)>,
}

impl ViaHeader {
    pub fn new(protocol: impl Into<String>, host: impl Into<String>, port: Option<u16>) -> Self {
        Self {
            protocol: protocol.into(),
            sent_by_host: host.into(),
            sent_by_port: port,
            parameters: Vec::new(),
        }
    }

    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.set_param("branch", Some(branch.into()));
        self
    }

    pub fn with_rport(mut self) -> Self {
        self.set_param("rport", None);
        self
    }

    pub fn branch(&self) -> Option<&str> {
        self.get_param("branch").flatten()
    }

    pub fn rport(&self) -> Option<Option<&str>> {
        self.get_param("rport")
    }

    pub fn received(&self) -> Option<&str> {
        self.get_param("received").flatten()
    }

    pub fn get_param(&self, key: &str) -> Option<Option<&str>> {
        self.parameters
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_deref())
    }

    pub fn set_param(&mut self, key: impl Into<String>, val: Option<String>) {
        let key = key.into();
        if let Some(pos) = self
            .parameters
            .iter()
            .position(|(k, _)| k.eq_ignore_ascii_case(&key))
        {
            self.parameters[pos].1 = val;
        } else {
            self.parameters.push((key, val));
        }
    }
}

impl fmt::Display for ViaHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.protocol, self.sent_by_host)?;
        if let Some(p) = self.sent_by_port {
            write!(f, ":{}", p)?;
        }
        for (k, v) in &self.parameters {
            write!(f, ";{}", k)?;
            if let Some(val) = v {
                write!(f, "={}", val)?;
            }
        }
        Ok(())
    }
}

impl FromStr for ViaHeader {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self> {
        let trimmed = s.trim();
        let (proto, rest) = trimmed.split_once(' ').ok_or_else(|| {
            ParseError::InvalidHeader(format!("Missing space in Via header: {}", trimmed))
        })?;

        let mut param_splits = rest.trim().split(';');
        let sent_by = param_splits.next().unwrap_or("").trim();
        let (host, port) = parse_host_port(sent_by)?;

        let mut parameters = Vec::new();
        for p in param_splits {
            let p = p.trim();
            if p.is_empty() {
                continue;
            }
            if let Some((k, v)) = p.split_once('=') {
                parameters.push((k.trim().to_string(), Some(v.trim().to_string())));
            } else {
                parameters.push((p.to_string(), None));
            }
        }

        Ok(ViaHeader {
            protocol: proto.to_string(),
            sent_by_host: host,
            sent_by_port: port,
            parameters,
        })
    }
}
