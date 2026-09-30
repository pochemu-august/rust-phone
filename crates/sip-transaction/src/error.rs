use sip_core::ParseError;
use sip_transport::TransportError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransactionError {
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),

    #[error("SIP Parse error: {0}")]
    Parse(#[from] ParseError),

    #[error("Transaction timeout")]
    Timeout,

    #[error("Transaction already terminated")]
    Terminated,

    #[error("Invalid transaction state transition from {0:?} on event {1}")]
    InvalidState(&'static str, &'static str),

    #[error("Missing Via header or branch parameter")]
    MissingBranch,
}

pub type Result<T> = std::result::Result<T, TransactionError>;
