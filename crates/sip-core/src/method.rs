use crate::error::ParseError;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Method {
    Invite,
    Ack,
    Bye,
    Cancel,
    Options,
    Register,
    Prack,
    Subscribe,
    Notify,
    Publish,
    Info,
    Refer,
    Message,
    Update,
    Custom(String),
}

impl Method {
    pub fn as_str(&self) -> &str {
        match self {
            Method::Invite => "INVITE",
            Method::Ack => "ACK",
            Method::Bye => "BYE",
            Method::Cancel => "CANCEL",
            Method::Options => "OPTIONS",
            Method::Register => "REGISTER",
            Method::Prack => "PRACK",
            Method::Subscribe => "SUBSCRIBE",
            Method::Notify => "NOTIFY",
            Method::Publish => "PUBLISH",
            Method::Info => "INFO",
            Method::Refer => "REFER",
            Method::Message => "MESSAGE",
            Method::Update => "UPDATE",
            Method::Custom(s) => s.as_str(),
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Method {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(ParseError::InvalidMethod("Empty method".into()));
        }
        Ok(match trimmed {
            "INVITE" => Method::Invite,
            "ACK" => Method::Ack,
            "BYE" => Method::Bye,
            "CANCEL" => Method::Cancel,
            "OPTIONS" => Method::Options,
            "REGISTER" => Method::Register,
            "PRACK" => Method::Prack,
            "SUBSCRIBE" => Method::Subscribe,
            "NOTIFY" => Method::Notify,
            "PUBLISH" => Method::Publish,
            "INFO" => Method::Info,
            "REFER" => Method::Refer,
            "MESSAGE" => Method::Message,
            "UPDATE" => Method::Update,
            other => Method::Custom(other.to_string()),
        })
    }
}
