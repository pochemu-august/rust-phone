use std::sync::Arc;
use tokio::sync::Mutex;

use sip_core::SipUri;
use sip_transaction::TransactionManager;
use sip_ua::{AccountConfig, RegistrationClient, RegistrationState};

use crate::error::Result;

pub struct Account {
    id: usize,
    config: AccountConfig,
    client: Arc<Mutex<RegistrationClient>>,
}

impl Account {
    pub fn new(
        id: usize,
        config: AccountConfig,
        contact_uri: SipUri,
        tx_manager: Arc<TransactionManager>,
    ) -> Self {
        let client = RegistrationClient::new(config.clone(), contact_uri, tx_manager);
        Self {
            id,
            config,
            client: Arc::new(Mutex::new(client)),
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn config(&self) -> &AccountConfig {
        &self.config
    }

    pub async fn state(&self) -> RegistrationState {
        self.client.lock().await.state.clone()
    }

    pub async fn register(&self) -> Result<u32> {
        let expires = self.client.lock().await.register().await?;
        Ok(expires)
    }

    pub async fn unregister(&self) -> Result<()> {
        self.client.lock().await.unregister().await?;
        Ok(())
    }
}
