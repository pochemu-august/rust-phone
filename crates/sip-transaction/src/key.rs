use sip_core::Method;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TransactionRole {
    Client,
    Server,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransactionKey {
    pub branch: String,
    pub method: Method,
    pub role: TransactionRole,
}

impl TransactionKey {
    pub fn new_client(branch: impl Into<String>, method: Method) -> Self {
        let m = if method == Method::Ack {
            Method::Invite
        } else {
            method
        };
        Self {
            branch: branch.into(),
            method: m,
            role: TransactionRole::Client,
        }
    }

    pub fn new_server(branch: impl Into<String>, method: Method) -> Self {
        let m = if method == Method::Ack {
            Method::Invite
        } else {
            method
        };
        Self {
            branch: branch.into(),
            method: m,
            role: TransactionRole::Server,
        }
    }
}
