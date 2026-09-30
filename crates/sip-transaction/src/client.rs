use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::time::sleep;
use tracing::{trace, warn};

use crate::error::Result;
use crate::timer::SipTimers;
use sip_core::{Header, Method, Request, Response, SipMessage};
use sip_transport::TransportManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    Calling,    // INVITE initial
    Trying,     // Non-INVITE initial
    Proceeding, // Received 1xx
    Completed,  // Received final response
    Terminated,
}

pub struct ClientTransactionHandle {
    pub branch: String,
    pub original_request: Request,
    pub target: SocketAddr,
    response_rx: mpsc::Receiver<Response>,
}

impl ClientTransactionHandle {
    pub async fn recv_response(&mut self) -> Option<Response> {
        self.response_rx.recv().await
    }
}

pub struct ClientTransaction {
    pub branch: String,
    pub request: Request,
    pub target: SocketAddr,
    pub state: Arc<Mutex<ClientState>>,
    response_tx: mpsc::Sender<Response>,
    transport: Arc<TransportManager>,
    timers: SipTimers,
}

impl ClientTransaction {
    pub fn spawn(
        request: Request,
        target: SocketAddr,
        branch: String,
        transport: Arc<TransportManager>,
        timers: SipTimers,
    ) -> (Self, ClientTransactionHandle) {
        let (response_tx, response_rx) = mpsc::channel(16);
        let is_invite = request.method == Method::Invite;
        let initial_state = if is_invite {
            ClientState::Calling
        } else {
            ClientState::Trying
        };
        let state = Arc::new(Mutex::new(initial_state));

        let tx = Self {
            branch: branch.clone(),
            request: request.clone(),
            target,
            state: Arc::clone(&state),
            response_tx,
            transport: Arc::clone(&transport),
            timers,
        };

        let handle = ClientTransactionHandle {
            branch,
            original_request: request.clone(),
            target,
            response_rx,
        };

        // Spawn retransmission and timer task
        let task_state = Arc::clone(&state);
        let task_req = request;
        let task_transport = transport;
        tokio::spawn(async move {
            let mut interval = timers.t1;
            let total_timeout = timers.timer_b();
            let start_time = tokio::time::Instant::now();

            // Initial send
            let msg = SipMessage::Request(task_req.clone());
            if let Err(e) = task_transport.send_to(&msg, target).await {
                warn!("Failed to send initial request: {}", e);
            }

            loop {
                sleep(interval).await;

                let cur_state = *task_state.lock().await;
                if cur_state == ClientState::Terminated || cur_state == ClientState::Completed {
                    break;
                }

                if start_time.elapsed() >= total_timeout {
                    warn!("Client transaction timeout reached");
                    *task_state.lock().await = ClientState::Terminated;
                    break;
                }

                // If INVITE is in Calling, retransmit
                // If Non-INVITE is in Trying or Proceeding, retransmit
                let should_retransmit = match cur_state {
                    ClientState::Calling => true,
                    ClientState::Trying | ClientState::Proceeding if !is_invite => true,
                    _ => false,
                };

                if should_retransmit {
                    trace!(
                        "Retransmitting request for branch {}",
                        task_req.headers.call_id().unwrap_or("-")
                    );
                    let _ = task_transport.send_to(&msg, target).await;
                    if is_invite {
                        interval *= 2;
                    } else {
                        interval = std::cmp::min(interval * 2, timers.t2);
                    }
                }
            }
        });

        (tx, handle)
    }

    pub async fn handle_response(&self, response: Response) -> Result<()> {
        let mut state = self.state.lock().await;
        if *state == ClientState::Terminated {
            return Ok(());
        }

        let is_invite = self.request.method == Method::Invite;

        if response.is_provisional() {
            // 1xx: Proceeding
            if *state == ClientState::Calling || *state == ClientState::Trying {
                *state = ClientState::Proceeding;
            }
            let _ = self.response_tx.send(response).await;
        } else if response.is_final() {
            if is_invite {
                if response.is_success() {
                    // 2xx response: RFC 3261 says 2xx ACK is handled by UA Dialog, transaction terminates
                    *state = ClientState::Terminated;
                    let _ = self.response_tx.send(response).await;
                } else {
                    // 300-699: Non-2xx final response. Transaction generates ACK automatically
                    *state = ClientState::Completed;
                    let ack = self.create_ack(&response);
                    let _ = self
                        .transport
                        .send_to(&SipMessage::Request(ack), self.target)
                        .await;

                    let _ = self.response_tx.send(response).await;

                    // Start timer D to terminate
                    let state_clone = Arc::clone(&self.state);
                    let timer_d = self.timers.t4;
                    tokio::spawn(async move {
                        sleep(timer_d).await;
                        *state_clone.lock().await = ClientState::Terminated;
                    });
                }
            } else {
                // Non-INVITE final response
                *state = ClientState::Completed;
                let _ = self.response_tx.send(response).await;

                // Timer K to terminate
                let state_clone = Arc::clone(&self.state);
                let timer_k = self.timers.timer_k();
                tokio::spawn(async move {
                    sleep(timer_k).await;
                    *state_clone.lock().await = ClientState::Terminated;
                });
            }
        }

        Ok(())
    }

    fn create_ack(&self, res: &Response) -> Request {
        let mut ack = Request::new(Method::Ack, self.request.uri.clone());

        // Copy top via
        if let Some(via) = self.request.headers.first_via() {
            ack.headers.push(Header::Via(via.clone()));
        }

        // From
        if let Some(from) = self.request.headers.from() {
            ack.headers.push(Header::From(from.clone()));
        }

        // To (with tag from response if any)
        if let Some(to) = res.headers.to() {
            ack.headers.push(Header::To(to.clone()));
        } else if let Some(to) = self.request.headers.to() {
            ack.headers.push(Header::To(to.clone()));
        }

        // Call-ID
        if let Some(cid) = self.request.headers.call_id() {
            ack.headers.push(Header::CallId(cid.to_string()));
        }

        // CSeq with Method::Ack and same sequence number
        if let Some(cseq) = self.request.headers.cseq() {
            ack.headers.push(Header::CSeq(sip_core::CSeqHeader::new(
                cseq.seq,
                Method::Ack,
            )));
        }

        ack.headers.push(Header::ContentLength(0));
        ack
    }
}
