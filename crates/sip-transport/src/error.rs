use sip_core::ParseError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("SIP Parse error: {0}")]
    Parse(#[from] ParseError),

    #[error("Address resolution error: {0}")]
    ResolveError(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Transport closed")]
    TransportClosed,
}

pub type Result<T> = std::result::Result<T, TransportError>;
