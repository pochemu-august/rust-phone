use rtp::prelude::*;

#[test]
fn test_rtp_packet_roundtrip() {
    let header = RtpHeader::new(8, 1234, 160000, 0x12345678);
    let payload = vec![0x11, 0x22, 0x33, 0x44, 0x55];

    let packet = RtpPacket::new(header, payload);
    let bytes = packet.to_bytes();

    let parsed = RtpPacket::parse(&bytes).unwrap();
    assert_eq!(parsed.header.version, 2);
    assert_eq!(parsed.header.payload_type, 8);
    assert_eq!(parsed.header.sequence_number, 1234);
    assert_eq!(parsed.header.timestamp, 160000);
    assert_eq!(parsed.header.ssrc, 0x12345678);
    assert_eq!(parsed.payload, vec![0x11, 0x22, 0x33, 0x44, 0x55]);
}

#[test]
fn test_jitter_buffer_reordering() {
    let mut jb = JitterBuffer::new(3, 10);

    let p1 = RtpPacket::new(RtpHeader::new(0, 100, 0, 1), vec![1]);
    let p2 = RtpPacket::new(RtpHeader::new(0, 101, 160, 1), vec![2]);
    let p3 = RtpPacket::new(RtpHeader::new(0, 102, 320, 1), vec![3]);

    // Push out of order: 100, then 102, then 101
    jb.push(p1);
    jb.push(p3);
    assert_eq!(jb.pop(), None); // Prebuffer count is 3, only 2 packets so far

    jb.push(p2); // 3rd packet arrived

    // Now it should pop in sequential order: 100, 101, 102
    let popped1 = jb.pop().expect("Expected packet 100");
    assert_eq!(popped1.header.sequence_number, 100);

    let popped2 = jb.pop().expect("Expected packet 101");
    assert_eq!(popped2.header.sequence_number, 101);

    let popped3 = jb.pop().expect("Expected packet 102");
    assert_eq!(popped3.header.sequence_number, 102);

    assert_eq!(jb.pop(), None);
}
