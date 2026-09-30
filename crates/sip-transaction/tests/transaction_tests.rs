use sip_core::prelude::*;
use sip_transaction::prelude::*;
use sip_transport::TransportManager;
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_non_invite_transaction_roundtrip() {
    let (tx_net1, rx_net1) = mpsc::channel(16);
    let (tx_net2, rx_net2) = mpsc::channel(16);

    let tm1 = Arc::new(
        TransportManager::new_udp("127.0.0.1:0".parse().unwrap(), tx_net1)
            .await
            .unwrap(),
    );
    let tm2 = Arc::new(
        TransportManager::new_udp("127.0.0.1:0".parse().unwrap(), tx_net2)
            .await
            .unwrap(),
    );

    let (server_disp1, _rx_server1) = mpsc::channel(16);
    let (server_disp2, mut rx_server2) = mpsc::channel(16);

    let timers = SipTimers::fast_test();

    let client_mgr = TransactionManager::new(tm1, rx_net1, server_disp1, timers);
    let server_mgr = TransactionManager::new(tm2, rx_net2, server_disp2, timers);

    // Client prepares OPTIONS request
    let uri = parse_uri("sip:receiver@127.0.0.1").unwrap();
    let req = Request::new(Method::Options, uri)
        .with_header(Header::From(
            parse_name_addr("<sip:caller@127.0.0.1>;tag=c1").unwrap(),
        ))
        .with_header(Header::To(
            parse_name_addr("<sip:receiver@127.0.0.1>").unwrap(),
        ))
        .with_header(Header::CallId("tx-test-options-1".to_string()))
        .with_header(Header::CSeq(CSeqHeader::new(1, Method::Options)));

    // Send request from client to server
    let mut handle = client_mgr
        .send_request(req, server_mgr.transport().local_addr())
        .await
        .unwrap();

    // Server receives ServerTransaction
    let server_tx = tokio::time::timeout(std::time::Duration::from_secs(2), rx_server2.recv())
        .await
        .expect("Timeout waiting for ServerTransaction")
        .expect("Channel closed");

    assert_eq!(server_tx.request.method, Method::Options);
    assert_eq!(
        server_tx.request.headers.call_id(),
        Some("tx-test-options-1")
    );

    // Server responds 200 OK
    let res = Response::reply_to(&server_tx.request, 200, "OK");
    server_tx.respond(res).await.unwrap();

    // Client receives response through handle
    let client_res =
        tokio::time::timeout(std::time::Duration::from_secs(2), handle.recv_response())
            .await
            .expect("Timeout waiting for client response")
            .expect("Channel closed");

    assert_eq!(client_res.status_code, 200);
    assert_eq!(client_res.reason_phrase, "OK");
}

#[tokio::test]
async fn test_invite_transaction_with_provisional() {
    let (tx_net1, rx_net1) = mpsc::channel(16);
    let (tx_net2, rx_net2) = mpsc::channel(16);

    let tm1 = Arc::new(
        TransportManager::new_udp("127.0.0.1:0".parse().unwrap(), tx_net1)
            .await
            .unwrap(),
    );
    let tm2 = Arc::new(
        TransportManager::new_udp("127.0.0.1:0".parse().unwrap(), tx_net2)
            .await
            .unwrap(),
    );

    let (server_disp1, _rx_server1) = mpsc::channel(16);
    let (server_disp2, mut rx_server2) = mpsc::channel(16);

    let timers = SipTimers::fast_test();

    let client_mgr = TransactionManager::new(tm1, rx_net1, server_disp1, timers);
    let server_mgr = TransactionManager::new(tm2, rx_net2, server_disp2, timers);

    let uri = parse_uri("sip:receiver@127.0.0.1").unwrap();
    let req = Request::new(Method::Invite, uri)
        .with_header(Header::From(
            parse_name_addr("<sip:caller@127.0.0.1>;tag=c2").unwrap(),
        ))
        .with_header(Header::To(
            parse_name_addr("<sip:receiver@127.0.0.1>").unwrap(),
        ))
        .with_header(Header::CallId("tx-test-invite-1".to_string()))
        .with_header(Header::CSeq(CSeqHeader::new(1, Method::Invite)));

    let mut handle = client_mgr
        .send_request(req, server_mgr.transport().local_addr())
        .await
        .unwrap();

    let server_tx = tokio::time::timeout(std::time::Duration::from_secs(2), rx_server2.recv())
        .await
        .expect("Timeout waiting for server tx")
        .expect("Channel closed");

    // Server sends 180 Ringing
    let ringing = Response::reply_to(&server_tx.request, 180, "Ringing");
    server_tx.respond(ringing).await.unwrap();

    // Client receives 180 Ringing
    let res1 = handle.recv_response().await.expect("Expected 180");
    assert_eq!(res1.status_code, 180);

    // Server sends 200 OK
    let ok = Response::reply_to(&server_tx.request, 200, "OK");
    server_tx.respond(ok).await.unwrap();

    // Client receives 200 OK
    let res2 = handle.recv_response().await.expect("Expected 200");
    assert_eq!(res2.status_code, 200);
}

#[tokio::test]
async fn test_invite_failure_auto_ack() {
    let (tx_net1, rx_net1) = mpsc::channel(16);
    let (tx_net2, rx_net2) = mpsc::channel(16);

    let tm1 = Arc::new(
        TransportManager::new_udp("127.0.0.1:0".parse().unwrap(), tx_net1)
            .await
            .unwrap(),
    );
    let tm2 = Arc::new(
        TransportManager::new_udp("127.0.0.1:0".parse().unwrap(), tx_net2)
            .await
            .unwrap(),
    );

    let (server_disp1, _rx_server1) = mpsc::channel(16);
    let (server_disp2, mut rx_server2) = mpsc::channel(16);

    let timers = SipTimers::fast_test();

    let client_mgr = TransactionManager::new(tm1, rx_net1, server_disp1, timers);
    let server_mgr = TransactionManager::new(tm2, rx_net2, server_disp2, timers);

    let uri = parse_uri("sip:receiver@127.0.0.1").unwrap();
    let req = Request::new(Method::Invite, uri)
        .with_header(Header::From(
            parse_name_addr("<sip:caller@127.0.0.1>;tag=c3").unwrap(),
        ))
        .with_header(Header::To(
            parse_name_addr("<sip:receiver@127.0.0.1>").unwrap(),
        ))
        .with_header(Header::CallId("tx-test-busy-1".to_string()))
        .with_header(Header::CSeq(CSeqHeader::new(1, Method::Invite)));

    let mut handle = client_mgr
        .send_request(req, server_mgr.transport().local_addr())
        .await
        .unwrap();

    let server_tx = rx_server2.recv().await.expect("Expected server tx");

    // Server responds with 486 Busy Here
    let busy = Response::reply_to(&server_tx.request, 486, "Busy Here");
    server_tx.respond(busy).await.unwrap();

    // Client receives 486 Busy Here
    let res = handle.recv_response().await.expect("Expected 486 response");
    assert_eq!(res.status_code, 486);

    // Wait a brief moment for the automatic ACK to arrive at server
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Server transaction state should have transitioned to Confirmed on receiving auto-ACK!
    let st = *server_tx.state.lock().await;
    assert!(st == ServerState::Confirmed || st == ServerState::Terminated);
}
