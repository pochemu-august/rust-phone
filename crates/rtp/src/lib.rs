pub mod error;
pub mod jitter;
pub mod packet;

pub use error::{Result, RtpError};
pub use jitter::{JitterBuffer, seq_greater};
pub use packet::{RtpHeader, RtpPacket};

pub mod prelude {
    pub use crate::error::{Result, RtpError};
    pub use crate::jitter::{JitterBuffer, seq_greater};
    pub use crate::packet::{RtpHeader, RtpPacket};
}
