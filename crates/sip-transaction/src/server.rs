use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::trace;

use crate::error::{Result, TransactionError};
use crate::timer::SipTimers;
use sip_core::{Method, Request, Response, SipMessage};
use sip_transport::TransportManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerState {
    Proceeding,
    Completed,
    Confirmed,
    Terminated,
}

pub struct ServerTransaction {
    pub branch: String,
    pub request: Request,
    pub peer_addr: SocketAddr,
    pub state: Arc<Mutex<ServerState>>,
    pub last_response: Arc<Mutex<Option<Response>>>,
    transport: Arc<TransportManager>,
    timers: SipTimers,
}

impl ServerTransaction {
    pub fn new(
        request: Request,
        peer_addr: SocketAddr,
        branch: String,
        transport: Arc<TransportManager>,
        timers: SipTimers,
    ) -> Self {
        Self {
            branch,
            request,
            peer_addr,
            state: Arc::new(Mutex::new(ServerState::Proceeding)),
            last_response: Arc::new(Mutex::new(None)),
            transport,
            timers,
        }
    }

    pub async fn respond(&self, response: Response) -> Result<()> {
        let mut state = self.state.lock().await;
        if *state == ServerState::Terminated {
            return Err(TransactionError::Terminated);
        }

        let is_invite = self.request.method == Method::Invite;
        let is_final = response.is_final();

        // Send response via transport
        let msg = SipMessage::Response(response.clone());
        self.transport.send_to(&msg, self.peer_addr).await?;

        *self.last_response.lock().await = Some(response.clone());

        if is_final {
            if is_invite {
                if response.status_code >= 200 && response.status_code < 300 {
                    // 2xx response for INVITE terminates server transaction immediately (UA handles it)
                    *state = ServerState::Terminated;
                } else {
                    // 300-699 for INVITE enters Completed, waits for ACK
                    *state = ServerState::Completed;

                    let state_clone = Arc::clone(&self.state);
                    let timer_h = self.timers.timer_b();
                    tokio::spawn(async move {
                        sleep(timer_h).await;
                        *state_clone.lock().await = ServerState::Terminated;
                    });
                }
            } else {
                // Non-INVITE final response enters Completed, waits Timer J
                *state = ServerState::Completed;

                let state_clone = Arc::clone(&self.state);
                let timer_j = self.timers.timer_b();
                tokio::spawn(async move {
                    sleep(timer_j).await;
                    *state_clone.lock().await = ServerState::Terminated;
                });
            }
        } else {
            *state = ServerState::Proceeding;
        }

        Ok(())
    }

    pub async fn handle_ack(&self) -> Result<()> {
        let mut state = self.state.lock().await;
        if *state == ServerState::Completed {
            *state = ServerState::Confirmed;
            let state_clone = Arc::clone(&self.state);
            let timer_i = self.timers.t4;
            tokio::spawn(async move {
                sleep(timer_i).await;
                *state_clone.lock().await = ServerState::Terminated;
            });
        }
        Ok(())
    }

    pub async fn handle_retransmit(&self) -> Result<()> {
        let last = self.last_response.lock().await.clone();
        if let Some(res) = last {
            trace!("Resending last response to {}", self.peer_addr);
            self.transport
                .send_to(&SipMessage::Response(res), self.peer_addr)
                .await?;
        }
        Ok(())
    }
}
