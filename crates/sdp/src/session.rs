use crate::error::{Result, SdpError};
use crate::media::{Attribute, Connection, Media};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub username: String,
    pub sess_id: u64,
    pub sess_version: u64,
    pub net_type: String,
    pub addr_type: String,
    pub unicast_address: String,
}

impl Origin {
    pub fn new(
        username: impl Into<String>,
        sess_id: u64,
        sess_version: u64,
        address: impl Into<String>,
    ) -> Self {
        Self {
            username: username.into(),
            sess_id,
            sess_version,
            net_type: "IN".to_string(),
            addr_type: "IP4".to_string(),
            unicast_address: address.into(),
        }
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "o={} {} {} {} {} {}",
            self.username,
            self.sess_id,
            self.sess_version,
            self.net_type,
            self.addr_type,
            self.unicast_address
        )
    }
}

impl FromStr for Origin {
    type Err = SdpError;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() < 6 {
            return Err(SdpError::MalformedLine(format!("o={}", s)));
        }
        let sess_id = parts[1]
            .parse::<u64>()
            .map_err(|_| SdpError::InvalidInteger("sess_id", parts[1].to_string()))?;
        let sess_version = parts[2]
            .parse::<u64>()
            .map_err(|_| SdpError::InvalidInteger("sess_version", parts[2].to_string()))?;

        Ok(Self {
            username: parts[0].to_string(),
            sess_id,
            sess_version,
            net_type: parts[3].to_string(),
            addr_type: parts[4].to_string(),
            unicast_address: parts[5].to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Time {
    pub start: u64,
    pub stop: u64,
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t={} {}", self.start, self.stop)
    }
}

impl FromStr for Time {
    type Err = SdpError;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() < 2 {
            return Err(SdpError::MalformedLine(format!("t={}", s)));
        }
        let start = parts[0]
            .parse::<u64>()
            .map_err(|_| SdpError::InvalidInteger("start_time", parts[0].to_string()))?;
        let stop = parts[1]
            .parse::<u64>()
            .map_err(|_| SdpError::InvalidInteger("stop_time", parts[1].to_string()))?;

        Ok(Self { start, stop })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDescription {
    pub version: u32,
    pub origin: Origin,
    pub session_name: String,
    pub connection: Option<Connection>,
    pub time: Time,
    pub attributes: Vec<Attribute>,
    pub medias: Vec<Media>,
}

impl SessionDescription {
    pub fn new(origin: Origin, session_name: impl Into<String>) -> Self {
        Self {
            version: 0,
            origin,
            session_name: session_name.into(),
            connection: None,
            time: Time::default(),
            attributes: Vec::new(),
            medias: Vec::new(),
        }
    }

    pub fn with_connection(mut self, conn: Connection) -> Self {
        self.connection = Some(conn);
        self
    }

    pub fn with_media(mut self, media: Media) -> Self {
        self.medias.push(media);
        self
    }

    pub fn with_attribute(mut self, attr: Attribute) -> Self {
        self.attributes.push(attr);
        self
    }

    pub fn audio_media(&self) -> Option<&Media> {
        self.medias
            .iter()
            .find(|m| m.media_type.eq_ignore_ascii_case("audio"))
    }

    pub fn audio_media_mut(&mut self) -> Option<&mut Media> {
        self.medias
            .iter_mut()
            .find(|m| m.media_type.eq_ignore_ascii_case("audio"))
    }
}

impl fmt::Display for SessionDescription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v={}\r\n", self.version)?;
        write!(f, "{}\r\n", self.origin)?;
        write!(f, "s={}\r\n", self.session_name)?;
        if let Some(conn) = &self.connection {
            write!(f, "{}\r\n", conn)?;
        }
        write!(f, "{}\r\n", self.time)?;
        for attr in &self.attributes {
            write!(f, "{}\r\n", attr)?;
        }
        for media in &self.medias {
            write!(f, "{}\r\n", media)?;
        }
        Ok(())
    }
}

impl FromStr for SessionDescription {
    type Err = SdpError;

    fn from_str(s: &str) -> Result<Self> {
        parse_sdp(s)
    }
}

pub fn parse_sdp(input: &str) -> Result<SessionDescription> {
    let mut version = None;
    let mut origin = None;
    let mut session_name = None;
    let mut connection = None;
    let mut time = None;
    let mut session_attrs = Vec::new();
    let mut medias: Vec<Media> = Vec::new();

    let mut current_media: Option<Media> = None;

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let (key, val) = match line.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => continue,
        };

        match key {
            "v" => {
                let v = val
                    .parse::<u32>()
                    .map_err(|_| SdpError::InvalidInteger("version", val.to_string()))?;
                if v != 0 {
                    return Err(SdpError::InvalidVersion(val.to_string()));
                }
                version = Some(v);
            }
            "o" => {
                origin = Some(Origin::from_str(val)?);
            }
            "s" => {
                session_name = Some(val.to_string());
            }
            "c" => {
                let conn = Connection::from_str(val)?;
                if let Some(m) = current_media.as_mut() {
                    m.connection = Some(conn);
                } else {
                    connection = Some(conn);
                }
            }
            "t" => {
                time = Some(Time::from_str(val)?);
            }
            "a" => {
                let attr = Attribute::from_str(val)?;
                if let Some(m) = current_media.as_mut() {
                    m.attributes.push(attr);
                } else {
                    session_attrs.push(attr);
                }
            }
            "m" => {
                // If we already had a media block in progress, push it
                if let Some(m) = current_media.take() {
                    medias.push(m);
                }

                // Parse media line: m=audio 4000 RTP/AVP 0 8 101
                let parts: Vec<&str> = val.split_whitespace().collect();
                if parts.len() < 3 {
                    return Err(SdpError::MalformedLine(format!("m={}", val)));
                }

                let media_type = parts[0].to_string();
                let port_str = parts[1];
                let (port, port_count) = if let Some((p, pc)) = port_str.split_once('/') {
                    let port = p
                        .parse::<u16>()
                        .map_err(|_| SdpError::InvalidInteger("port", p.to_string()))?;
                    let port_count = pc
                        .parse::<u16>()
                        .map_err(|_| SdpError::InvalidInteger("port_count", pc.to_string()))?;
                    (port, Some(port_count))
                } else {
                    let port = port_str
                        .parse::<u16>()
                        .map_err(|_| SdpError::InvalidInteger("port", port_str.to_string()))?;
                    (port, None)
                };

                let proto = parts[2].to_string();
                let formats: Vec<String> = parts[3..].iter().map(|f| f.to_string()).collect();

                current_media = Some(Media {
                    media_type,
                    port,
                    port_count,
                    proto,
                    formats,
                    connection: None,
                    attributes: Vec::new(),
                });
            }
            _ => {
                // Ignore other lines like u=, e=, p=, b=, etc. for now
            }
        }
    }

    if let Some(m) = current_media.take() {
        medias.push(m);
    }

    let origin = origin.ok_or(SdpError::MissingField("o="))?;
    let session_name = session_name.unwrap_or_else(|| "-".to_string());
    let time = time.unwrap_or_default();

    Ok(SessionDescription {
        version: version.unwrap_or(0),
        origin,
        session_name,
        connection,
        time,
        attributes: session_attrs,
        medias,
    })
}
