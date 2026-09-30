use std::net::SocketAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallState {
    Null,
    Calling,
    Incoming,
    Early,
    Connecting,
    Confirmed,
    Disconnected,
}

impl std::fmt::Display for CallState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallState::Null => write!(f, "NULL"),
            CallState::Calling => write!(f, "CALLING"),
            CallState::Incoming => write!(f, "INCOMING"),
            CallState::Early => write!(f, "EARLY (Ringing)"),
            CallState::Connecting => write!(f, "CONNECTING"),
            CallState::Confirmed => write!(f, "CONFIRMED (Active)"),
            CallState::Disconnected => write!(f, "DISCONNECTED"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum PjsuaEvent {
    IncomingCall {
        call_id: String,
        from_uri: String,
        to_uri: String,
    },
    CallStateChanged {
        call_id: String,
        state: CallState,
        status_code: u16,
        reason: String,
    },
    MediaStreamReady {
        call_id: String,
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
        codec: String,
    },
    RegistrationStateChanged {
        acc_id: usize,
        state: sip_ua::RegistrationState,
        status_code: u16,
        reason: String,
    },
}
