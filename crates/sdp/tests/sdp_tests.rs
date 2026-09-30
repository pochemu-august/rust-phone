use sdp::prelude::*;

#[test]
fn test_parse_sdp_sample() {
    let raw = concat!(
        "v=0\r\n",
        "o=alice 2890844526 2890844526 IN IP4 host.atlanta.example.com\r\n",
        "s=-\r\n",
        "c=IN IP4 192.0.2.101\r\n",
        "t=0 0\r\n",
        "m=audio 49172 RTP/AVP 0 8 101\r\n",
        "a=rtpmap:0 PCMU/8000\r\n",
        "a=rtpmap:8 PCMA/8000\r\n",
        "a=rtpmap:101 telephone-event/8000\r\n",
        "a=fmtp:101 0-16\r\n",
        "a=sendrecv\r\n"
    );

    let sdp = parse_sdp(raw).unwrap();
    assert_eq!(sdp.version, 0);
    assert_eq!(sdp.origin.username, "alice");
    assert_eq!(sdp.origin.sess_id, 2890844526);
    assert_eq!(sdp.connection.as_ref().unwrap().address, "192.0.2.101");
    assert_eq!(sdp.medias.len(), 1);

    let audio = sdp.audio_media().unwrap();
    assert_eq!(audio.port, 49172);
    assert_eq!(audio.proto, "RTP/AVP");
    assert_eq!(audio.formats, vec!["0", "8", "101"]);

    let rtmaps = audio.rtmaps();
    assert_eq!(rtmaps.len(), 3);
    assert_eq!(rtmaps[0].payload_type, 0);
    assert_eq!(rtmaps[0].encoding_name, "PCMU");
    assert_eq!(rtmaps[0].clock_rate, 8000);
}

#[test]
fn test_sdp_negotiation() {
    let offer_raw = concat!(
        "v=0\r\n",
        "o=caller 100 100 IN IP4 10.0.0.1\r\n",
        "s=-\r\n",
        "c=IN IP4 10.0.0.1\r\n",
        "t=0 0\r\n",
        "m=audio 10000 RTP/AVP 0 8 101\r\n",
        "a=rtpmap:0 PCMU/8000\r\n",
        "a=rtpmap:8 PCMA/8000\r\n",
        "a=rtpmap:101 telephone-event/8000\r\n",
        "a=sendrecv\r\n"
    );

    let offer = parse_sdp(offer_raw).unwrap();

    // Local only supports PCMA (8) and PCMU (0)
    let local_origin = Origin::new("callee", 200, 200, "10.0.0.2");
    let mut local_caps = SessionDescription::new(local_origin, "-");
    local_caps.connection = Some(Connection::new_ip4("10.0.0.2"));

    let local_audio = Media::new_audio(20000, vec!["8", "0"])
        .with_attribute(Attribute::rtpmap(8, "PCMA", 8000, None))
        .with_attribute(Attribute::rtpmap(0, "PCMU", 8000, None))
        .with_attribute(Attribute::flag("sendrecv"));

    local_caps.medias.push(local_audio);

    let answer = negotiate_answer(&offer, &local_caps).unwrap();
    let ans_audio = answer.audio_media().unwrap();

    assert_eq!(ans_audio.port, 20000);
    // Negotiated formats are in the offer's order where both agree: 0, 8
    assert_eq!(ans_audio.formats, vec!["0", "8"]);
    assert_eq!(answer.connection.as_ref().unwrap().address, "10.0.0.2");
}
