use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::lookup_host;
use tokio::sync::mpsc;
use tracing::info;

use crate::error::{Result, TransportError};
use crate::udp::UdpTransport;
use sip_core::SipMessage;

pub struct TransportManager {
    udp: Arc<UdpTransport>,
}

impl TransportManager {
    pub async fn new_udp(
        bind_addr: SocketAddr,
        incoming_tx: mpsc::Sender<(SipMessage, SocketAddr)>,
    ) -> Result<Self> {
        let udp = UdpTransport::bind(bind_addr, incoming_tx).await?;
        info!("SIP UDP transport listening on {}", udp.local_addr());
        Ok(Self { udp: Arc::new(udp) })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.udp.local_addr()
    }

    pub async fn send_to(&self, msg: &SipMessage, target: SocketAddr) -> Result<()> {
        self.udp.send_message(msg, target).await
    }

    pub async fn send_to_host(
        &self,
        msg: &SipMessage,
        host: &str,
        port: u16,
    ) -> Result<SocketAddr> {
        let addr_str = format!("{}:{}", host, port);
        let mut addrs = lookup_host(&addr_str)
            .await
            .map_err(|e| TransportError::ResolveError(format!("{}: {}", addr_str, e)))?;

        let target = addrs.next().ok_or_else(|| {
            TransportError::ResolveError(format!("No IP resolved for {}", addr_str))
        })?;

        self.send_to(msg, target).await?;
        Ok(target)
    }
}
