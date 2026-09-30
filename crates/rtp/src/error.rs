use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum RtpError {
    #[error("RTP packet too short: expected at least {expected}, got {got}")]
    PacketTooShort { expected: usize, got: usize },

    #[error("Invalid RTP version: expected 2, got {0}")]
    InvalidVersion(u8),

    #[error("Malformed extension header")]
    MalformedExtension,

    #[error("Invalid padding length")]
    InvalidPadding,
}

pub type Result<T> = std::result::Result<T, RtpError>;
