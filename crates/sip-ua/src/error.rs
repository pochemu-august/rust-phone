use sip_core::ParseError;
use sip_transaction::TransactionError;
use sip_transport::TransportError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UaError {
    #[error("Transaction error: {0}")]
    Transaction(#[from] TransactionError),

    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),

    #[error("SIP Parse error: {0}")]
    Parse(#[from] ParseError),

    #[error("Authentication error: {0}")]
    AuthFailed(String),

    #[error("Registration failed: {0}")]
    RegistrationFailed(String),

    #[error("Dialog not found: {0}")]
    DialogNotFound(String),

    #[error("Dialog in invalid state: {0}")]
    InvalidDialogState(String),

    #[error("Missing header: {0}")]
    MissingHeader(&'static str),
}

pub type Result<T> = std::result::Result<T, UaError>;
