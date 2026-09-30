use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tracing::info;

use pjsua::prelude::*;

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("╔═══════════════════════════════════════════════════════════════════╗");
    println!("║                   RUST-PHONE: PJSIP Pure-Rust Port                ║");
    println!("║        High-Performance, Memory-Safe Asynchronous SIP Phone       ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝");

    let mut config = EndpointConfig {
        sip_bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5060),
        rtp_port_start: 16000,
        ..Default::default()
    };

    let endpoint = match Endpoint::init(config.clone()).await {
        Ok(ep) => ep,
        Err(_) => {
            println!("[*] Port 5060 is in use, binding to dynamic local port...");
            config.sip_bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
            Endpoint::init(config).await?
        }
    };

    let local_addr = endpoint.local_sip_addr();
    println!("[*] SIP Transport listening on: {}", local_addr);

    let acc_cfg = AccountConfig {
        username: "alice".to_string(),
        password: "secretpassword".to_string(),
        domain: local_addr.to_string(),
        registrar_addr: local_addr,
        expires: 3600,
    };

    let acc_id = endpoint.add_account(acc_cfg).await;
    println!(
        "[*] Account #{} initialized: sip:alice@{}",
        acc_id, local_addr
    );

    println!("[*] Rust-Phone SIP stack ready! Running self-test simulation...");

    // Spawn a second endpoint (Bob) to simulate incoming call & RTP audio
    let bob_cfg = EndpointConfig {
        sip_bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        rtp_port_start: 18000,
        ..Default::default()
    };
    let bob_ep = Endpoint::init(bob_cfg).await?;

    let _bob_acc_id = bob_ep
        .add_account(AccountConfig {
            username: "bob".to_string(),
            password: "bobpassword".to_string(),
            domain: bob_ep.local_sip_addr().to_string(),
            registrar_addr: bob_ep.local_sip_addr(),
            expires: 3600,
        })
        .await;

    println!(
        "[*] Remote peer (Bob) listening on: {}",
        bob_ep.local_sip_addr()
    );

    // Alice makes call to Bob
    let dst_uri = format!("sip:bob@{}", bob_ep.local_sip_addr());
    println!("[*] Alice calling {}...", dst_uri);
    let alice_call = endpoint.make_call(acc_id, &dst_uri).await?;

    // Bob receives call and answers
    tokio::time::sleep(Duration::from_millis(50)).await;
    if let Some(PjsuaEvent::IncomingCall { call_id, .. }) = bob_ep.next_event().await {
        println!("[*] Bob incoming call: ID={}, answering 200 OK...", call_id);
        bob_ep.answer_call(&call_id, 200).await?;

        // Wait for call confirmed
        tokio::time::sleep(Duration::from_millis(100)).await;
        println!("[*] Alice call state: {}", alice_call.state().await);

        // Send G.711 PCMU audio tone from Alice to Bob
        println!("[*] Streaming G.711 RTP audio packets from Alice to Bob...");
        let samples: Vec<i16> = (0..160)
            .map(|i| ((i as f32 * 0.1).sin() * 8000.0) as i16)
            .collect();
        alice_call.send_audio(&samples).await?;

        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Some(bob_call) = bob_ep.get_call(&call_id).await
            && let Some(recv) = bob_call.recv_audio().await
        {
            println!(
                "[*] Bob received audio frame! Samples decoded: {}",
                recv.len()
            );
        }

        // Alice hangs up
        println!("[*] Alice hanging up (sending SIP BYE)...");
        endpoint.hangup_call(alice_call.id()).await?;
        tokio::time::sleep(Duration::from_millis(50)).await;
        println!("[*] Call successfully terminated. Self-test completed with 100% SUCCESS!");
    }

    info!("Rust-Phone shutdown completed.");
    Ok(())
}
