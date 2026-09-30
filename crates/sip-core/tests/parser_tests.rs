use sip_core::prelude::*;

#[test]
fn test_parse_simple_sip_uri() {
    let uri = parse_sip_uri("sip:localhost").unwrap();
    assert_eq!(uri.scheme, Scheme::Sip);
    assert_eq!(uri.host, "localhost");
    assert_eq!(uri.user, None);
    assert_eq!(uri.port, None);
    assert_eq!(uri.to_string(), "sip:localhost");

    let uri = parse_sip_uri("sip:user@localhost").unwrap();
    assert_eq!(uri.user.as_deref(), Some("user"));
    assert_eq!(uri.host, "localhost");

    let uri = parse_sip_uri("sip:user:password@localhost:5060").unwrap();
    assert_eq!(uri.user.as_deref(), Some("user"));
    assert_eq!(uri.password.as_deref(), Some("password"));
    assert_eq!(uri.host, "localhost");
    assert_eq!(uri.port, Some(5060));
}

#[test]
fn test_parse_uri_with_params() {
    let uri = parse_sip_uri("sip:localhost;transport=tcp;lr;maddr=127.0.0.1").unwrap();
    assert_eq!(uri.host, "localhost");
    assert_eq!(uri.transport(), Some("tcp"));
    assert!(uri.is_lr());
    assert_eq!(uri.get_param("maddr"), Some(Some("127.0.0.1")));
}

#[test]
fn test_parse_name_addr() {
    let na = parse_name_addr("\"Alice Smith\" <sip:alice@atlanta.com>;tag=1928301774").unwrap();
    assert_eq!(na.display_name.as_deref(), Some("Alice Smith"));
    assert_eq!(na.tag(), Some("1928301774"));
    let sip = na.uri.sip().unwrap();
    assert_eq!(sip.user.as_deref(), Some("alice"));
    assert_eq!(sip.host, "atlanta.com");

    let na_star = parse_name_addr("*").unwrap();
    assert_eq!(na_star.to_string(), "*");
}

#[test]
fn test_pjsip_msg_normal_invite() {
    let raw = concat!(
        "INVITE sip:user@foo SIP/2.0\n",
        "from: Hi I'm Joe <sip:joe.user@bar.otherdomain.com>;tag=123457890123456\r",
        "To: Fellow User <sip:user@foo.bar.domain.com>\r\n",
        "Call-ID: 12345678901234567890@bar\r\n",
        "Content-Length: 0\r\n",
        "CSeq: 123456 INVITE\n",
        "Contact: <sip:joe@bar> ; q=0.5;expires=3600,sip:user@host;q=0.500\r",
        "  ,sip:user2@host2\n",
        "Content-Type: text/html ; charset=ISO-8859-4\r",
        "Route: <sip:bigbox3.site3.atlanta.com;lr>,\r\n",
        "  <sip:server10.biloxi.com;lr>\r",
        "Record-Route: <sip:server10.biloxi.com>,\r\n",
        "  <sip:bigbox3.site3.atlanta.com;lr>\n",
        "v: SIP/2.0/SCTP bigbox3.site3.atlanta.com;branch=z9hG4bK77ef4c230\n",
        "Via: SIP/2.0/UDP pc33.atlanta.com;branch=z9hG4bKnashds8\n",
        " ;received=192.0.2.1\r\n",
        "Via: SIP/2.0/UDP 10.2.1.1, SIP/2.0/TCP 192.168.1.1\n",
        "Organization: \r",
        "Max-Forwards: 70\n",
        "X-Header: \r\n",
        "P-Associated-URI:\r\n",
        "\r\n"
    );

    let msg = parse_message(raw.as_bytes()).unwrap();
    assert!(msg.is_request());
    let req = msg.request().unwrap();

    assert_eq!(req.method, Method::Invite);
    assert_eq!(req.uri.to_string(), "sip:user@foo");
    assert_eq!(req.headers.call_id(), Some("12345678901234567890@bar"));
    assert_eq!(req.headers.cseq().unwrap().seq, 123456);
    assert_eq!(req.headers.cseq().unwrap().method, Method::Invite);
    assert_eq!(req.headers.from().unwrap().tag(), Some("123457890123456"));
    assert_eq!(
        req.headers.to().unwrap().display_name.as_deref(),
        Some("Fellow User")
    );
    assert_eq!(req.headers.content_length(), Some(0));
    assert_eq!(req.headers.max_forwards(), Some(70));

    // Check Vias (including compact 'v:' and comma separated)
    let vias = req.headers.vias();
    assert_eq!(vias.len(), 4);
    assert_eq!(vias[0].protocol, "SIP/2.0/SCTP");
    assert_eq!(vias[0].sent_by_host, "bigbox3.site3.atlanta.com");
    assert_eq!(vias[0].branch(), Some("z9hG4bK77ef4c230"));

    assert_eq!(vias[1].protocol, "SIP/2.0/UDP");
    assert_eq!(vias[1].sent_by_host, "pc33.atlanta.com");
    assert_eq!(vias[1].branch(), Some("z9hG4bKnashds8"));
    assert_eq!(vias[1].received(), Some("192.0.2.1"));

    // Check Contacts
    let contacts = req.headers.contacts();
    assert_eq!(contacts.len(), 3);
}

#[test]
fn test_pjsip_msg_response() {
    let raw = b"SIP/2.0 200 OK\r\n\
Via: SIP/2.0/UDP bigbox3.site3.atlanta.com;branch=z9hG4bK77ef4c2312983.1;received=192.0.2.2\r\n\
Max-Forwards: 70\r\n\
To: Bob <sip:bob@biloxi.com>;tag=a6c85cf\r\n\
From: Alice <sip:alice@atlanta.com>;tag=1928301774\r\n\
Call-ID: a84b4c76e66710@pc33.atlanta.com\r\n\
CSeq: 314159 INVITE\r\n\
Contact: <sips:bob@192.0.2.4>\r\n\
Content-Type: application/sdp\r\n\
Content-Length: 12\r\n\
\r\n\
v=0\r\no=alice";

    let msg = parse_message(raw).unwrap();
    assert!(msg.is_response());
    let res = msg.response().unwrap();

    assert_eq!(res.status_code, 200);
    assert_eq!(res.reason_phrase, "OK");
    assert!(res.is_success());
    assert_eq!(
        res.headers.call_id(),
        Some("a84b4c76e66710@pc33.atlanta.com")
    );
    assert_eq!(res.headers.to().unwrap().tag(), Some("a6c85cf"));
    assert_eq!(res.body, b"v=0\r\no=alice");
}

#[test]
fn test_reply_to() {
    let req = Request::new(Method::Invite, parse_uri("sip:bob@biloxi.com").unwrap())
        .with_header(Header::Via(
            ViaHeader::new("SIP/2.0/UDP", "192.168.1.10", Some(5060)).with_branch("z9hG4bK12345"),
        ))
        .with_header(Header::From(
            parse_name_addr("<sip:alice@atlanta.com>;tag=1111").unwrap(),
        ))
        .with_header(Header::To(parse_name_addr("<sip:bob@biloxi.com>").unwrap()))
        .with_header(Header::CallId("test-call-1@local".to_string()))
        .with_header(Header::CSeq(CSeqHeader::new(1, Method::Invite)));

    let res = Response::reply_to(&req, 200, "OK");
    assert_eq!(res.status_code, 200);
    assert_eq!(res.headers.call_id(), Some("test-call-1@local"));
    assert_eq!(
        res.headers.first_via().unwrap().branch(),
        Some("z9hG4bK12345")
    );
}

#[test]
fn test_parse_uri_ipv6() {
    let uri = parse_sip_uri("sip:[2001:db8::1]:5060;transport=tcp").unwrap();
    assert_eq!(uri.host, "[2001:db8::1]");
    assert_eq!(uri.port, Some(5060));
    assert_eq!(uri.transport(), Some("tcp"));
}

#[test]
fn test_compact_headers() {
    let raw = concat!(
        "OPTIONS sip:proxy.com SIP/2.0\r\n",
        "v: SIP/2.0/UDP 10.0.0.1:5060;branch=z9hG4bKabc\r\n",
        "f: <sip:caller@test.com>;tag=123\r\n",
        "t: <sip:proxy.com>\r\n",
        "i: my-call-id-999\r\n",
        "m: <sip:caller@10.0.0.1:5060>\r\n",
        "c: application/sdp\r\n",
        "l: 0\r\n",
        "s: Test Subject\r\n",
        "k: 100rel, replaces\r\n",
        "\r\n"
    );

    let msg = parse_message(raw.as_bytes()).unwrap();
    let req = msg.request().unwrap();

    assert_eq!(req.method, Method::Options);
    assert_eq!(req.headers.call_id(), Some("my-call-id-999"));
    assert_eq!(req.headers.from().unwrap().tag(), Some("123"));
    assert_eq!(req.headers.content_type(), Some("application/sdp"));
    assert_eq!(req.headers.content_length(), Some(0));
    assert_eq!(req.headers.contacts().len(), 1);
}

#[test]
fn test_auth_header() {
    let raw = concat!(
        "SIP/2.0 401 Unauthorized\r\n",
        "WWW-Authenticate: Digest realm=\"biloxi.com\", qop=\"auth,auth-int\", nonce=\"dcd98b7102dd2f0e8b11d0f600bfb0c093\", opaque=\"5ccc069c403ebaf9f0171e9517f30e41\"\r\n",
        "Content-Length: 0\r\n",
        "\r\n"
    );

    let msg = parse_message(raw.as_bytes()).unwrap();
    let res = msg.response().unwrap();

    assert_eq!(res.status_code, 401);
    let auth = res
        .headers
        .iter()
        .find_map(|h| match h {
            Header::WwwAuthenticate(a) => Some(a),
            _ => None,
        })
        .unwrap();

    assert_eq!(auth.scheme, "Digest");
    assert_eq!(auth.realm.as_deref(), Some("biloxi.com"));
    assert_eq!(
        auth.nonce.as_deref(),
        Some("dcd98b7102dd2f0e8b11d0f600bfb0c093")
    );
    assert_eq!(
        auth.opaque.as_deref(),
        Some("5ccc069c403ebaf9f0171e9517f30e41")
    );
    assert_eq!(auth.qop.as_deref(), Some("auth,auth-int"));
}

#[test]
fn test_invalid_syntax() {
    assert!(parse_sip_uri("invalid_uri").is_err());
    assert!(parse_message(b"INVALID").is_err());
    assert!(parse_message(b"SIP/2.0 NOT_A_CODE OK\r\n\r\n").is_err());
}
