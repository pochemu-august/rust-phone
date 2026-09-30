use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::trace;

use crate::dialog::{Dialog, DialogId};

#[derive(Default)]
pub struct DialogManager {
    dialogs: Arc<Mutex<HashMap<DialogId, Dialog>>>,
}

impl DialogManager {
    pub fn new() -> Self {
        Self {
            dialogs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn insert(&self, dialog: Dialog) {
        trace!("Inserting dialog {:?}", dialog.id);
        self.dialogs.lock().await.insert(dialog.id.clone(), dialog);
    }

    pub async fn get(&self, id: &DialogId) -> Option<Dialog> {
        self.dialogs.lock().await.get(id).cloned()
    }

    pub async fn remove(&self, id: &DialogId) -> Option<Dialog> {
        trace!("Removing dialog {:?}", id);
        self.dialogs.lock().await.remove(id)
    }

    pub async fn count(&self) -> usize {
        self.dialogs.lock().await.len()
    }
}
