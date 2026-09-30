use media::prelude::*;

#[test]
fn test_g711_pcma_roundtrip() {
    let original = [0i16, 100, -100, 1000, -1000, 15000, -15000, 30000, -30000];
    let encoded = pcm16_to_pcma(&original);
    let decoded = pcma_to_pcm16(&encoded);

    assert_eq!(encoded.len(), original.len());
    assert_eq!(decoded.len(), original.len());

    // G.711 is lossy 8-bit companding, verify that decoded values are very close to original
    for (orig, dec) in original.iter().zip(decoded.iter()) {
        let diff = (orig - dec).abs();
        let relative = if orig.abs() > 0 {
            diff as f32 / orig.abs() as f32
        } else {
            0.0
        };
        assert!(
            relative < 0.15 || diff < 32,
            "Diff too large: orig={}, dec={}, diff={}",
            orig,
            dec,
            diff
        );
    }
}

#[test]
fn test_g711_pcmu_roundtrip() {
    let original = [0i16, 100, -100, 1000, -1000, 15000, -15000, 30000, -30000];
    let encoded = pcm16_to_pcmu(&original);
    let decoded = pcmu_to_pcm16(&encoded);

    for (orig, dec) in original.iter().zip(decoded.iter()) {
        let diff = (orig - dec).abs();
        let relative = if orig.abs() > 0 {
            diff as f32 / orig.abs() as f32
        } else {
            0.0
        };
        assert!(
            relative < 0.15 || diff < 32,
            "Diff too large: orig={}, dec={}, diff={}",
            orig,
            dec,
            diff
        );
    }
}

#[test]
fn test_dtmf_generation() {
    let frame = generate_dtmf('5', 8000, 20).expect("DTMF generation failed");
    assert_eq!(frame.sample_rate, 8000);
    assert_eq!(frame.channels, 1);
    assert_eq!(frame.samples.len(), 160); // 20ms at 8000Hz = 160 samples
    assert_eq!(frame.duration_ms(), 20);

    // Frame should not be complete silence
    let non_zero = frame.samples.iter().any(|&s| s != 0);
    assert!(non_zero);
}

#[tokio::test]
async fn test_rtp_audio_stream_roundtrip() {
    let mut sender = RtpAudioStream::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let receiver = RtpAudioStream::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();

    let target_addr = receiver.local_addr();

    // Generate 3 audio frames (20ms each)
    let f1 = generate_sine(440.0, 8000, 20, 0.5);
    let f2 = generate_sine(440.0, 8000, 20, 0.5);
    let f3 = generate_sine(440.0, 8000, 20, 0.5);

    sender.send_frame(&f1, 8, target_addr).await.unwrap();
    sender.send_frame(&f2, 8, target_addr).await.unwrap();
    sender.send_frame(&f3, 8, target_addr).await.unwrap();

    // Wait for network delivery and prebuffering
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let received = receiver.pop_frame().await;
    assert!(received.is_some());
    let r_frame = received.unwrap();
    assert_eq!(r_frame.samples.len(), 160);
}
