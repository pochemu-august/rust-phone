use sip_core::prelude::*;
use sip_transaction::prelude::*;
use sip_transport::TransportManager;
use sip_ua::prelude::*;
use std::sync::Arc;
use tokio::sync::mpsc;

#[test]
fn test_rfc2617_digest_vector() {
    // RFC 2617 standard test vector:
    // username = "Mufasa"
    // realm = "testrealm@host.com"
    // password = "Circle Of Life"
    // uri = "/dir/index.html"
    // nonce = "dcd98b7102dd2f0e8b11d0f600bfb0c093"
    // cnonce = "0a4f113b"
    // nc = "00000001"
    // qop = "auth"
    // expected response = "6629fae49393a05397450978507c4ef1"

    let response = calculate_digest_response(
        "Mufasa",
        "Circle Of Life",
        "testrealm@host.com",
        "GET",
        "/dir/index.html",
        "dcd98b7102dd2f0e8b11d0f600bfb0c093",
        Some("00000001"),
        Some("0a4f113b"),
        Some("auth"),
    );

    assert_eq!(response, "6629fae49393a05397450978507c4ef1");
}

#[test]
fn test_dialog_lifecycle() {
    let req = Request::new(Method::Invite, parse_uri("sip:bob@biloxi.com").unwrap())
        .with_header(Header::Via(
            ViaHeader::new("SIP/2.0/UDP", "192.168.1.10", Some(5060)).with_branch("z9hG4bK-d1"),
        ))
        .with_header(Header::From(
            parse_name_addr("<sip:alice@atlanta.com>;tag=alice-tag-1").unwrap(),
        ))
        .with_header(Header::To(parse_name_addr("<sip:bob@biloxi.com>").unwrap()))
        .with_header(Header::CallId("dlg-call-id-1@atlanta".to_string()))
        .with_header(Header::CSeq(CSeqHeader::new(10, Method::Invite)));

    let mut res = Response::reply_to(&req, 200, "OK");
    // Add remote tag and Contact
    res.headers.set_to_tag("bob-tag-9");
    res.headers.push(Header::Contact(
        parse_name_addr("<sip:bob@192.168.1.50:5060>").unwrap(),
    ));

    let mut dialog = Dialog::create_uac(&req, &res).unwrap();
    assert_eq!(dialog.state, DialogState::Confirmed);
    assert_eq!(dialog.id.call_id, "dlg-call-id-1@atlanta");
    assert_eq!(dialog.id.local_tag, "alice-tag-1");
    assert_eq!(dialog.id.remote_tag, "bob-tag-9");
    assert_eq!(
        dialog.remote_target.to_string(),
        "sip:bob@192.168.1.50:5060"
    );

    // Generate in-dialog ACK for 200 OK
    let ack = dialog.create_ack_for_2xx(&res).unwrap();
    assert_eq!(ack.method, Method::Ack);
    assert_eq!(ack.uri.to_string(), "sip:bob@192.168.1.50:5060");
    assert_eq!(ack.headers.cseq().unwrap().seq, 10);
    assert_eq!(ack.headers.cseq().unwrap().method, Method::Ack);

    // Generate in-dialog BYE
    let bye = dialog.create_bye();
    assert_eq!(bye.method, Method::Bye);
    assert_eq!(bye.headers.cseq().unwrap().seq, 11);
    assert_eq!(bye.headers.cseq().unwrap().method, Method::Bye);
    assert_eq!(dialog.state, DialogState::Terminated);
}

#[tokio::test]
async fn test_registration_client_with_auth_challenge() {
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

    let client_tm = TransactionManager::new(tm1, rx_net1, server_disp1, timers);
    let registrar_tm = TransactionManager::new(tm2, rx_net2, server_disp2, timers);

    let registrar_addr = registrar_tm.transport().local_addr();

    // Spawn mock registrar handling initial REGISTER (challenges with 401) and subsequent authenticated REGISTER (responds 200)
    tokio::spawn(async move {
        // 1. Receive unauthenticated REGISTER
        let server_tx1 = rx_server2
            .recv()
            .await
            .expect("Expected unauthenticated REGISTER");
        assert_eq!(server_tx1.request.method, Method::Register);

        // Challenge with 401 Unauthorized
        let mut challenge_401 = Response::reply_to(&server_tx1.request, 401, "Unauthorized");
        let mut auth_challenge = AuthHeader::digest("sip.example.com", "nonce-12345");
        auth_challenge.qop = Some("auth".to_string());
        challenge_401
            .headers
            .push(Header::WwwAuthenticate(auth_challenge));

        server_tx1.respond(challenge_401).await.unwrap();

        // 2. Receive authenticated REGISTER
        let server_tx2 = rx_server2
            .recv()
            .await
            .expect("Expected authenticated REGISTER");
        assert_eq!(server_tx2.request.method, Method::Register);

        // Verify that Authorization header is present
        let auth_hdr = server_tx2.request.headers.iter().find_map(|h| match h {
            Header::Authorization(a) => Some(a),
            _ => None,
        });
        assert!(auth_hdr.is_some());
        let auth = auth_hdr.unwrap();
        assert_eq!(auth.realm.as_deref(), Some("sip.example.com"));
        assert_eq!(auth.username.as_deref(), Some("alice"));
        assert!(auth.response.is_some());

        // Respond with 200 OK and Expires: 3600
        let mut ok_200 = Response::reply_to(&server_tx2.request, 200, "OK");
        ok_200.headers.push(Header::Expires(3600));
        server_tx2.respond(ok_200).await.unwrap();
    });

    let config = AccountConfig {
        username: "alice".to_string(),
        password: "secretpassword".to_string(),
        domain: "sip.example.com".to_string(),
        registrar_addr,
        expires: 3600,
    };

    let contact_uri = SipUri::new("127.0.0.1")
        .with_user("alice")
        .with_port(client_tm.transport().local_addr().port());
    let mut client = RegistrationClient::new(config, contact_uri, client_tm);

    let expires = client.register().await.unwrap();
    assert_eq!(expires, 3600);
    assert!(matches!(client.state, RegistrationState::Registered { .. }));
}
