use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use pjsua::prelude::*;

pub struct AppState {
    pub endpoint: Arc<Mutex<Option<Arc<Endpoint>>>>,
    pub active_calls: Arc<Mutex<HashMap<String, Arc<Call>>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            endpoint: Arc::new(Mutex::new(None)),
            active_calls: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
