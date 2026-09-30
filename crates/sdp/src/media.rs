use crate::error::{Result, SdpError};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    pub net_type: String,  // typically "IN"
    pub addr_type: String, // "IP4" or "IP6"
    pub address: String,
}

impl Connection {
    pub fn new_ip4(address: impl Into<String>) -> Self {
        Self {
            net_type: "IN".to_string(),
            addr_type: "IP4".to_string(),
            address: address.into(),
        }
    }
}

impl fmt::Display for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "c={} {} {}", self.net_type, self.addr_type, self.address)
    }
}

impl FromStr for Connection {
    type Err = SdpError;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(SdpError::MalformedLine(format!("c={}", s)));
        }
        Ok(Self {
            net_type: parts[0].to_string(),
            addr_type: parts[1].to_string(),
            address: parts[2].to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: Option<String>,
}

impl Attribute {
    pub fn new(name: impl Into<String>, value: Option<impl Into<String>>) -> Self {
        Self {
            name: name.into(),
            value: value.map(|v| v.into()),
        }
    }

    pub fn flag(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: None,
        }
    }

    pub fn rtpmap(pt: u8, encoding: &str, clock_rate: u32, channels: Option<u32>) -> Self {
        let val = if let Some(ch) = channels {
            format!("{} {}/{}/{}", pt, encoding, clock_rate, ch)
        } else {
            format!("{} {}/{}", pt, encoding, clock_rate)
        };
        Self::new("rtpmap", Some(val))
    }

    pub fn parse_rtpmap(&self) -> Option<RtpMap> {
        if !self.name.eq_ignore_ascii_case("rtpmap") {
            return None;
        }
        let val = self.value.as_deref()?;
        let (pt_str, rest) = val.split_once(' ')?;
        let pt = pt_str.parse::<u8>().ok()?;
        let mut enc_splits = rest.split('/');
        let encoding_name = enc_splits.next()?.to_string();
        let clock_rate = enc_splits.next()?.parse::<u32>().ok()?;
        let channels = enc_splits.next().and_then(|c| c.parse::<u32>().ok());

        Some(RtpMap {
            payload_type: pt,
            encoding_name,
            clock_rate,
            channels,
        })
    }
}

impl fmt::Display for Attribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a={}", self.name)?;
        if let Some(val) = &self.value {
            write!(f, ":{}", val)?;
        }
        Ok(())
    }
}

impl FromStr for Attribute {
    type Err = SdpError;

    fn from_str(s: &str) -> Result<Self> {
        match s.split_once(':') {
            Some((name, val)) => Ok(Attribute::new(name.trim(), Some(val.trim()))),
            None => Ok(Attribute::flag(s.trim())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtpMap {
    pub payload_type: u8,
    pub encoding_name: String,
    pub clock_rate: u32,
    pub channels: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    pub media_type: String, // "audio", "video"
    pub port: u16,
    pub port_count: Option<u16>,
    pub proto: String,        // "RTP/AVP", "RTP/SAVP"
    pub formats: Vec<String>, // e.g. ["0", "8", "101"]
    pub connection: Option<Connection>,
    pub attributes: Vec<Attribute>,
}

impl Media {
    pub fn new_audio(port: u16, formats: Vec<impl Into<String>>) -> Self {
        Self {
            media_type: "audio".to_string(),
            port,
            port_count: None,
            proto: "RTP/AVP".to_string(),
            formats: formats.into_iter().map(|f| f.into()).collect(),
            connection: None,
            attributes: Vec::new(),
        }
    }

    pub fn with_attribute(mut self, attr: Attribute) -> Self {
        self.attributes.push(attr);
        self
    }

    pub fn with_connection(mut self, conn: Connection) -> Self {
        self.connection = Some(conn);
        self
    }

    pub fn find_attribute(&self, name: &str) -> Option<&Attribute> {
        self.attributes
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
    }

    pub fn rtmaps(&self) -> Vec<RtpMap> {
        self.attributes
            .iter()
            .filter_map(|a| a.parse_rtpmap())
            .collect()
    }
}

impl fmt::Display for Media {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "m={} {}", self.media_type, self.port)?;
        if let Some(pc) = self.port_count {
            write!(f, "/{}", pc)?;
        }
        write!(f, " {} {}", self.proto, self.formats.join(" "))?;
        if let Some(conn) = &self.connection {
            write!(f, "\r\n{}", conn)?;
        }
        for attr in &self.attributes {
            write!(f, "\r\n{}", attr)?;
        }
        Ok(())
    }
}
