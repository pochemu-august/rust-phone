pub mod auth;
pub mod dialog;
pub mod dialog_manager;
pub mod error;
pub mod register;

pub use auth::{calculate_digest_response, create_auth_response, md5_hex};
pub use dialog::{Dialog, DialogId, DialogState};
pub use dialog_manager::DialogManager;
pub use error::{Result, UaError};
pub use register::{AccountConfig, RegistrationClient, RegistrationState};

pub mod prelude {
    pub use crate::auth::{calculate_digest_response, create_auth_response, md5_hex};
    pub use crate::dialog::{Dialog, DialogId, DialogState};
    pub use crate::dialog_manager::DialogManager;
    pub use crate::error::{Result, UaError};
    pub use crate::register::{AccountConfig, RegistrationClient, RegistrationState};
}
