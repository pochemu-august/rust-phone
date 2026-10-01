use pjsua::prelude::*;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_endpoint_init_and_account() {
    let config = EndpointConfig {
        sip_bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        rtp_port_start: 12000,
        ..Default::default()
    };

    let endpoint = Endpoint::init(config).await.unwrap();
    assert_ne!(endpoint.local_sip_addr().port(), 0);

    let acc_cfg = AccountConfig {
        username: "alice".to_string(),
        password: "secret".to_string(),
        domain: "127.0.0.1".to_string(),
        registrar_addr: "127.0.0.1:5060".parse().unwrap(),
        expires: 3600,
    };

    let acc_id = endpoint.add_account(acc_cfg).await;
    assert_eq!(acc_id, 0);

    let acc = endpoint.get_account(acc_id).await.unwrap();
    assert_eq!(acc.id(), 0);
    assert_eq!(acc.state().await, RegistrationState::Unregistered);
}

#[tokio::test]
async fn test_pjsua_call_and_media_flow() {
    // 1. Initialize Alice's Endpoint
    let alice_cfg = EndpointConfig {
        sip_bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        rtp_port_start: 14000,
        ..Default::default()
    };
    let alice_ep = Endpoint::init(alice_cfg).await.unwrap();

    let alice_acc_id = alice_ep
        .add_account(AccountConfig {
            username: "alice".to_string(),
            password: "secret".to_string(),
            domain: "127.0.0.1".to_string(),
            registrar_addr: "127.0.0.1:5060".parse().unwrap(),
            expires: 3600,
        })
        .await;

    // 2. Initialize Bob's Endpoint
    let bob_cfg = EndpointConfig {
        sip_bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        rtp_port_start: 15000,
        ..Default::default()
    };
    let bob_ep = Endpoint::init(bob_cfg).await.unwrap();

    let _bob_acc_id = bob_ep
        .add_account(AccountConfig {
            username: "bob".to_string(),
            password: "secret".to_string(),
            domain: "127.0.0.1".to_string(),
            registrar_addr: "127.0.0.1:5060".parse().unwrap(),
            expires: 3600,
        })
        .await;

    let bob_sip_addr = bob_ep.local_sip_addr();
    let bob_uri_str = format!("sip:bob@{}", bob_sip_addr);

    // 3. Alice calls Bob
    let alice_call = alice_ep
        .make_call(alice_acc_id, &bob_uri_str)
        .await
        .unwrap();
    let call_id = alice_call.id().to_string();

    // 4. Bob waits for incoming call
    let bob_incoming_event = timeout(Duration::from_secs(3), async {
        loop {
            if let Some(PjsuaEvent::IncomingCall { call_id: cid, .. }) = bob_ep.next_event().await {
                return cid;
            }
        }
    })
    .await
    .expect("Timeout waiting for Bob's incoming call");

    assert_eq!(bob_incoming_event, call_id);

    // 5. Bob answers call with 200 OK
    bob_ep.answer_call(&call_id, 200).await.unwrap();

    // 6. Wait for call confirmation on both ends
    let alice_confirmed = timeout(Duration::from_secs(3), async {
        loop {
            if let Some(PjsuaEvent::CallStateChanged {
                state: CallState::Confirmed,
                ..
            }) = alice_ep.next_event().await
            {
                return true;
            }
        }
    })
    .await
    .expect("Timeout waiting for Alice call confirmation");
    assert!(alice_confirmed);

    // 7. Verify Alice and Bob calls exist and are active
    let bob_call = bob_ep.get_call(&call_id).await.unwrap();
    assert_eq!(alice_call.state().await, CallState::Confirmed);

    // 8. RTP Media exchange: Alice sends audio to Bob (send 3 frames for 3-packet prebuffer)
    let pcm_samples: Vec<i16> = (0..160).map(|i| (i * 100) as i16).collect();
    for _ in 0..3 {
        alice_call.send_audio(&pcm_samples).await.unwrap();
    }

    // Receive audio on Bob's side
    let recv_audio = timeout(Duration::from_secs(3), async {
        loop {
            if let Some(samples) = bob_call.recv_audio().await
                && !samples.is_empty()
            {
                return samples;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Timeout waiting for audio on Bob side");

    assert_eq!(recv_audio.len(), 160);

    // 9. Alice hangs up
    alice_ep.hangup_call(&call_id).await.unwrap();

    // Bob receives Disconnected
    let bob_disconnected = timeout(Duration::from_secs(3), async {
        loop {
            if let Some(PjsuaEvent::CallStateChanged {
                state: CallState::Disconnected,
                ..
            }) = bob_ep.next_event().await
            {
                return true;
            }
        }
    })
    .await
    .expect("Timeout waiting for Bob disconnect");
    assert!(bob_disconnected);
}
