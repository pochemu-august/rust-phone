pub mod error;
pub mod media;
pub mod negotiate;
pub mod session;

pub use error::{Result, SdpError};
pub use media::{Attribute, Connection, Media, RtpMap};
pub use negotiate::negotiate_answer;
pub use session::{Origin, SessionDescription, Time, parse_sdp};

pub mod prelude {
    pub use crate::error::{Result, SdpError};
    pub use crate::media::{Attribute, Connection, Media, RtpMap};
    pub use crate::negotiate::negotiate_answer;
    pub use crate::session::{Origin, SessionDescription, Time, parse_sdp};
}
