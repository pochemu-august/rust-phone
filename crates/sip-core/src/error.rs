use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    #[error("Unexpected end of input: {0}")]
    UnexpectedEof(&'static str),

    #[error("Invalid SIP URI: {0}")]
    InvalidUri(String),

    #[error("Invalid SIP method: {0}")]
    InvalidMethod(String),

    #[error("Invalid SIP version: expected 'SIP/2.0', got '{0}'")]
    InvalidVersion(String),

    #[error("Invalid status code: '{0}'")]
    InvalidStatusCode(String),

    #[error("Invalid header format: '{0}'")]
    InvalidHeader(String),

    #[error("Invalid CSeq: '{0}'")]
    InvalidCSeq(String),

    #[error("Invalid Content-Length: '{0}'")]
    InvalidContentLength(String),

    #[error("Malformed start line: '{0}'")]
    MalformedStartLine(String),

    #[error("Syntax error: {0}")]
    SyntaxError(String),
}

pub type Result<T> = std::result::Result<T, ParseError>;
