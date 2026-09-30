use crate::error::{ParseError, Result};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthHeader {
    pub scheme: String, // typically "Digest"
    pub username: Option<String>,
    pub realm: Option<String>,
    pub nonce: Option<String>,
    pub uri: Option<String>,
    pub response: Option<String>,
    pub algorithm: Option<String>,
    pub cnonce: Option<String>,
    pub opaque: Option<String>,
    pub qop: Option<String>,
    pub nc: Option<String>,
    pub params: Vec<(String, String)>,
}

impl AuthHeader {
    pub fn new(scheme: impl Into<String>) -> Self {
        Self {
            scheme: scheme.into(),
            username: None,
            realm: None,
            nonce: None,
            uri: None,
            response: None,
            algorithm: None,
            cnonce: None,
            opaque: None,
            qop: None,
            nc: None,
            params: Vec::new(),
        }
    }

    pub fn digest(realm: impl Into<String>, nonce: impl Into<String>) -> Self {
        Self {
            scheme: "Digest".to_string(),
            username: None,
            realm: Some(realm.into()),
            nonce: Some(nonce.into()),
            uri: None,
            response: None,
            algorithm: None,
            cnonce: None,
            opaque: None,
            qop: None,
            nc: None,
            params: Vec::new(),
        }
    }
}

impl fmt::Display for AuthHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.scheme)?;
        let mut first = true;
        let mut write_item =
            |f: &mut fmt::Formatter<'_>, key: &str, val: &str, quoted: bool| -> fmt::Result {
                if first {
                    write!(f, " ")?;
                    first = false;
                } else {
                    write!(f, ", ")?;
                }
                if quoted {
                    write!(f, "{}=\"{}\"", key, val)?;
                } else {
                    write!(f, "{}={}", key, val)?;
                }
                Ok(())
            };

        if let Some(v) = &self.username {
            write_item(f, "username", v, true)?;
        }
        if let Some(v) = &self.realm {
            write_item(f, "realm", v, true)?;
        }
        if let Some(v) = &self.nonce {
            write_item(f, "nonce", v, true)?;
        }
        if let Some(v) = &self.uri {
            write_item(f, "uri", v, true)?;
        }
        if let Some(v) = &self.response {
            write_item(f, "response", v, true)?;
        }
        if let Some(v) = &self.algorithm {
            write_item(f, "algorithm", v, false)?;
        }
        if let Some(v) = &self.cnonce {
            write_item(f, "cnonce", v, true)?;
        }
        if let Some(v) = &self.opaque {
            write_item(f, "opaque", v, true)?;
        }
        if let Some(v) = &self.qop {
            write_item(f, "qop", v, false)?;
        }
        if let Some(v) = &self.nc {
            write_item(f, "nc", v, false)?;
        }
        for (k, v) in &self.params {
            write_item(f, k, v, true)?;
        }
        Ok(())
    }
}

impl FromStr for AuthHeader {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self> {
        let trimmed = s.trim();
        let (scheme, rest) = trimmed.split_once(' ').ok_or_else(|| {
            ParseError::InvalidHeader(format!("Invalid auth header: {}", trimmed))
        })?;

        let mut auth = AuthHeader::new(scheme.trim());

        // Parse comma-separated key=val, respecting quotes
        let mut chars = rest.trim().chars().peekable();
        while chars.peek().is_some() {
            // Skip leading whitespace and commas
            while let Some(&c) = chars.peek() {
                if c == ' ' || c == '\t' || c == ',' {
                    chars.next();
                } else {
                    break;
                }
            }
            if chars.peek().is_none() {
                break;
            }

            // Read key
            let mut key = String::new();
            while let Some(&c) = chars.peek() {
                if c == '=' || c == ',' || c == ' ' || c == '\t' {
                    break;
                }
                key.push(c);
                chars.next();
            }

            // Skip spaces until '='
            while let Some(&c) = chars.peek() {
                if c == '=' {
                    chars.next();
                    break;
                }
                chars.next();
            }

            // Skip spaces before value
            while let Some(&c) = chars.peek() {
                if c == ' ' {
                    chars.next();
                } else {
                    break;
                }
            }

            // Read val (quoted or token)
            let mut val = String::new();
            if let Some(&'"') = chars.peek() {
                chars.next(); // consume opening quote
                while let Some(c) = chars.next() {
                    if c == '"' {
                        break;
                    }
                    if c == '\\'
                        && let Some(next_c) = chars.next()
                    {
                        val.push(next_c);
                        continue;
                    }
                    val.push(c);
                }
            } else {
                while let Some(&c) = chars.peek() {
                    if c == ',' {
                        break;
                    }
                    val.push(c);
                    chars.next();
                }
                val = val.trim().to_string();
            }

            let key_str = key.trim();
            if !key_str.is_empty() {
                match key_str.to_ascii_lowercase().as_str() {
                    "username" => auth.username = Some(val),
                    "realm" => auth.realm = Some(val),
                    "nonce" => auth.nonce = Some(val),
                    "uri" => auth.uri = Some(val),
                    "response" => auth.response = Some(val),
                    "algorithm" => auth.algorithm = Some(val),
                    "cnonce" => auth.cnonce = Some(val),
                    "opaque" => auth.opaque = Some(val),
                    "qop" => auth.qop = Some(val),
                    "nc" => auth.nc = Some(val),
                    _ => auth.params.push((key_str.to_string(), val)),
                }
            }

            // Skip to next comma
            while let Some(&c) = chars.peek() {
                if c == ',' {
                    chars.next();
                    break;
                }
                chars.next();
            }
        }

        Ok(auth)
    }
}
