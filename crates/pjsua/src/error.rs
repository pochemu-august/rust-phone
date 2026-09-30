use thiserror::Error;

pub type Result<T> = std::result::Result<T, PjsuaError>;

#[derive(Error, Debug)]
pub enum PjsuaError {
    #[error("SIP core error: {0}")]
    SipCore(#[from] sip_core::ParseError),

    #[error("SIP transport error: {0}")]
    SipTransport(#[from] sip_transport::TransportError),

    #[error("SIP transaction error: {0}")]
    SipTransaction(#[from] sip_transaction::TransactionError),

    #[error("SIP UA error: {0}")]
    SipUa(#[from] sip_ua::UaError),

    #[error("SDP error: {0}")]
    Sdp(#[from] sdp::SdpError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Account not found: {0}")]
    AccountNotFound(usize),

    #[error("Call not found: {0}")]
    CallNotFound(String),

    #[error("Invalid call state for action: current {current:?}, expected {expected}")]
    InvalidCallState {
        current: String,
        expected: &'static str,
    },

    #[error("Media error: {0}")]
    Media(String),

    #[error("Destination error: {0}")]
    InvalidDestination(String),

    #[error("Call rejected or terminated by remote: code {code}, reason {reason}")]
    CallFailed { code: u16, reason: String },

    #[error("Endpoint already shut down")]
    ShutDown,
}
