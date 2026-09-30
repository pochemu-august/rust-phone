use sip_core::prelude::*;
use sip_transport::prelude::*;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_udp_send_receive_roundtrip() {
    let (tx1, mut rx1) = mpsc::channel(16);
    let (tx2, mut rx2) = mpsc::channel(16);

    let t1 = TransportManager::new_udp("127.0.0.1:0".parse().unwrap(), tx1)
        .await
        .unwrap();
    let t2 = TransportManager::new_udp("127.0.0.1:0".parse().unwrap(), tx2)
        .await
        .unwrap();

    let target2 = t2.local_addr();

    // Create an OPTIONS request
    let uri = parse_uri("sip:target@127.0.0.1").unwrap();
    let req = Request::new(Method::Options, uri)
        .with_header(Header::Via(
            ViaHeader::new("SIP/2.0/UDP", "127.0.0.1", Some(t1.local_addr().port()))
                .with_branch("z9hG4bKtest123"),
        ))
        .with_header(Header::From(
            parse_name_addr("<sip:sender@127.0.0.1>;tag=tag1").unwrap(),
        ))
        .with_header(Header::To(
            parse_name_addr("<sip:target@127.0.0.1>").unwrap(),
        ))
        .with_header(Header::CallId("test-call-id-roundtrip".to_string()))
        .with_header(Header::CSeq(CSeqHeader::new(1, Method::Options)));

    let msg = SipMessage::Request(req);

    // Send from t1 to t2
    t1.send_to(&msg, target2).await.unwrap();

    // Receive on t2
    let (received_msg, peer_addr) =
        tokio::time::timeout(std::time::Duration::from_secs(2), rx2.recv())
            .await
            .expect("Timeout waiting for message")
            .expect("Channel closed");

    assert_eq!(peer_addr, t1.local_addr());
    assert!(received_msg.is_request());
    let req = received_msg.request().unwrap();
    assert_eq!(req.method, Method::Options);
    assert_eq!(req.headers.call_id(), Some("test-call-id-roundtrip"));

    // Send 200 OK reply from t2 to t1
    let reply = Response::reply_to(req, 200, "OK");
    let reply_msg = SipMessage::Response(reply);

    t2.send_to(&reply_msg, peer_addr).await.unwrap();

    // Receive on t1
    let (received_reply, from_peer) =
        tokio::time::timeout(std::time::Duration::from_secs(2), rx1.recv())
            .await
            .expect("Timeout waiting for reply")
            .expect("Channel closed");

    assert_eq!(from_peer, t2.local_addr());
    assert!(received_reply.is_response());
    let res = received_reply.response().unwrap();
    assert_eq!(res.status_code, 200);
    assert_eq!(res.reason_phrase, "OK");
    assert_eq!(res.headers.call_id(), Some("test-call-id-roundtrip"));
}
