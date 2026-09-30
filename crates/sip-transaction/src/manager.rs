use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, trace, warn};

use crate::client::{ClientTransaction, ClientTransactionHandle};
use crate::error::Result;
use crate::key::TransactionKey;
use crate::server::ServerTransaction;
use crate::timer::SipTimers;
use sip_core::{Header, Method, Request, Response, SipMessage, ViaHeader};
use sip_transport::TransportManager;

pub struct TransactionManager {
    transport: Arc<TransportManager>,
    timers: SipTimers,
    client_txs: Arc<Mutex<HashMap<TransactionKey, Arc<ClientTransaction>>>>,
    server_txs: Arc<Mutex<HashMap<TransactionKey, Arc<ServerTransaction>>>>,
    ack_dispatcher: Arc<Mutex<Option<mpsc::Sender<Request>>>>,
}

impl TransactionManager {
    pub fn new(
        transport: Arc<TransportManager>,
        mut incoming_rx: mpsc::Receiver<(SipMessage, SocketAddr)>,
        server_tx_dispatcher: mpsc::Sender<Arc<ServerTransaction>>,
        timers: SipTimers,
    ) -> Arc<Self> {
        let client_txs = Arc::new(Mutex::new(HashMap::new()));
        let server_txs = Arc::new(Mutex::new(HashMap::new()));
        let ack_dispatcher = Arc::new(Mutex::new(None));

        let mgr = Arc::new(Self {
            transport,
            timers,
            client_txs: Arc::clone(&client_txs),
            server_txs: Arc::clone(&server_txs),
            ack_dispatcher,
        });

        // Spawn central dispatch loop
        let loop_mgr = Arc::clone(&mgr);
        tokio::spawn(async move {
            while let Some((msg, peer_addr)) = incoming_rx.recv().await {
                match msg {
                    SipMessage::Response(res) => {
                        loop_mgr.handle_incoming_response(res).await;
                    }
                    SipMessage::Request(req) => {
                        loop_mgr
                            .handle_incoming_request(req, peer_addr, &server_tx_dispatcher)
                            .await;
                    }
                }
            }
        });

        mgr
    }

    pub fn transport(&self) -> &Arc<TransportManager> {
        &self.transport
    }

    pub async fn set_ack_dispatcher(&self, dispatcher: mpsc::Sender<Request>) {
        *self.ack_dispatcher.lock().await = Some(dispatcher);
    }

    pub async fn send_request(
        &self,
        mut req: Request,
        target: SocketAddr,
    ) -> Result<ClientTransactionHandle> {
        // Ensure top Via has branch parameter with RFC 3261 magic cookie z9hG4bK
        let branch = if let Some(via) = req.headers.first_via() {
            if let Some(b) = via.branch() {
                b.to_string()
            } else {
                let gen_branch = generate_branch();
                let mut new_via = via.clone();
                new_via.set_param("branch", Some(gen_branch.clone()));
                // Replace top via
                self.replace_top_via(&mut req, new_via);
                gen_branch
            }
        } else {
            let gen_branch = generate_branch();
            let via = ViaHeader::new(
                "SIP/2.0/UDP",
                self.transport.local_addr().ip().to_string(),
                Some(self.transport.local_addr().port()),
            )
            .with_branch(gen_branch.clone());
            req.headers.push(Header::Via(via));
            gen_branch
        };

        let key = TransactionKey::new_client(branch.clone(), req.method.clone());
        let (tx, handle) = ClientTransaction::spawn(
            req,
            target,
            branch,
            Arc::clone(&self.transport),
            self.timers,
        );

        let tx_arc = Arc::new(tx);
        self.client_txs
            .lock()
            .await
            .insert(key, Arc::clone(&tx_arc));

        Ok(handle)
    }

    async fn handle_incoming_response(&self, res: Response) {
        let branch = match res.headers.first_via().and_then(|v| v.branch()) {
            Some(b) => b,
            None => {
                warn!("Received response without Via branch, dropping");
                return;
            }
        };

        let method = match res.headers.cseq().map(|c| c.method.clone()) {
            Some(m) => m,
            None => {
                warn!("Received response without CSeq method, dropping");
                return;
            }
        };

        let key = TransactionKey::new_client(branch, method);
        let client_tx = {
            let txs = self.client_txs.lock().await;
            txs.get(&key).cloned()
        };

        if let Some(tx) = client_tx {
            trace!(
                "Matching response {} for client transaction {}",
                res.status_code, key.branch
            );
            if let Err(e) = tx.handle_response(res).await {
                warn!("Error handling response in transaction: {}", e);
            }
        } else {
            debug!("No client transaction found for response key {:?}", key);
        }
    }

    async fn handle_incoming_request(
        &self,
        req: Request,
        peer_addr: SocketAddr,
        server_tx_dispatcher: &mpsc::Sender<Arc<ServerTransaction>>,
    ) {
        let branch = match req.headers.first_via().and_then(|v| v.branch()) {
            Some(b) => b.to_string(),
            None => {
                warn!("Received request without Via branch from {}", peer_addr);
                return;
            }
        };

        let key = TransactionKey::new_server(branch.clone(), req.method.clone());

        let existing_tx = {
            let txs = self.server_txs.lock().await;
            txs.get(&key).cloned()
        };

        if let Some(tx) = existing_tx {
            if req.method == Method::Ack {
                trace!("Received ACK for server transaction {}", branch);
                let _ = tx.handle_ack().await;
            } else {
                trace!(
                    "Received retransmitted request for server transaction {}",
                    branch
                );
                let _ = tx.handle_retransmit().await;
            }
        } else {
            // New request
            if req.method == Method::Ack {
                // Stray ACK or 2xx ACK (handled by dialog/UA layer directly)
                debug!(
                    "Received ACK without existing non-2xx server transaction, branch: {}",
                    branch
                );
                if let Some(ref disp) = *self.ack_dispatcher.lock().await {
                    let _ = disp.send(req).await;
                }
                return;
            }

            let server_tx = Arc::new(ServerTransaction::new(
                req,
                peer_addr,
                branch,
                Arc::clone(&self.transport),
                self.timers,
            ));

            self.server_txs
                .lock()
                .await
                .insert(key, Arc::clone(&server_tx));

            if server_tx_dispatcher.send(server_tx).await.is_err() {
                debug!("Server transaction dispatcher dropped");
            }
        }
    }

    fn replace_top_via(&self, req: &mut Request, new_via: ViaHeader) {
        // Collect existing headers, replacing the first Via
        let mut replaced = false;
        let mut new_headers = sip_core::Headers::new();
        for h in req.headers.iter() {
            if !replaced && matches!(h, Header::Via(_)) {
                new_headers.push(Header::Via(new_via.clone()));
                replaced = true;
            } else {
                new_headers.push(h.clone());
            }
        }
        req.headers = new_headers;
    }
}

fn generate_branch() -> String {
    format!(
        "z9hG4bK{:x}{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        rand_simple()
    )
}

fn rand_simple() -> u32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    nanos ^ 0x5a5a5a5a
}
