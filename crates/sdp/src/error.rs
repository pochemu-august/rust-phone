use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum SdpError {
    #[error("Empty SDP input")]
    EmptyInput,

    #[error("Malformed line: '{0}'")]
    MalformedLine(String),

    #[error("Missing mandatory field: {0}")]
    MissingField(&'static str),

    #[error("Invalid version: '{0}' (expected 0)")]
    InvalidVersion(String),

    #[error("Invalid integer in field {0}: '{1}'")]
    InvalidInteger(&'static str, String),

    #[error("Negotiation failed: {0}")]
    NegotiationFailed(String),
}

pub type Result<T> = std::result::Result<T, SdpError>;
