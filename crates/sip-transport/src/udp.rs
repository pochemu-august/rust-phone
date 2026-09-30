use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{debug, error, trace, warn};

use crate::error::{Result, TransportError};
use sip_core::{SipMessage, parse_message};

pub struct UdpTransport {
    socket: Arc<UdpSocket>,
    local_addr: SocketAddr,
}

impl UdpTransport {
    /// Binds to the given local address and spawns a background receiver task
    pub async fn bind(
        bind_addr: SocketAddr,
        incoming_tx: mpsc::Sender<(SipMessage, SocketAddr)>,
    ) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        let local_addr = socket.local_addr()?;
        let socket = Arc::new(socket);

        // Spawn background receive loop
        let recv_socket = Arc::clone(&socket);
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65535];
            loop {
                match recv_socket.recv_from(&mut buf).await {
                    Ok((len, peer_addr)) => {
                        trace!("Received {} bytes from UDP {}", len, peer_addr);
                        match parse_message(&buf[..len]) {
                            Ok(msg) => {
                                if incoming_tx.send((msg, peer_addr)).await.is_err() {
                                    debug!("Incoming message receiver dropped, stopping UDP loop");
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse SIP message from {}: {}", peer_addr, e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("UDP receive error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(Self { socket, local_addr })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub async fn send_message(&self, msg: &SipMessage, target: SocketAddr) -> Result<()> {
        let bytes = msg.to_bytes();
        trace!("Sending {} bytes to UDP {}", bytes.len(), target);
        self.socket
            .send_to(&bytes, target)
            .await
            .map_err(TransportError::Io)?;
        Ok(())
    }
}
