use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use sdp::{Attribute, Connection, Media, Origin, SessionDescription};

pub fn create_audio_offer(
    local_ip: IpAddr,
    local_rtp_port: u16,
    session_id: u64,
) -> SessionDescription {
    let mut sdp = SessionDescription::new(
        Origin::new("rust-phone", session_id, session_id, local_ip.to_string()),
        "SIP Call",
    );

    sdp.connection = Some(Connection::new_ip4(local_ip.to_string()));

    let media = Media::new_audio(local_rtp_port, vec!["0", "8"])
        .with_attribute(Attribute::new("rtpmap", Some("0 PCMU/8000")))
        .with_attribute(Attribute::new("rtpmap", Some("8 PCMA/8000")))
        .with_attribute(Attribute::flag("sendrecv"));

    sdp.medias.push(media);
    sdp
}

pub fn create_audio_answer(
    local_ip: IpAddr,
    local_rtp_port: u16,
    session_id: u64,
    matched_format: &str,
) -> SessionDescription {
    create_audio_direction_answer(
        local_ip,
        local_rtp_port,
        session_id,
        matched_format,
        "sendrecv",
    )
}

pub fn create_audio_direction_answer(
    local_ip: IpAddr,
    local_rtp_port: u16,
    session_id: u64,
    matched_format: &str,
    direction: &str,
) -> SessionDescription {
    let mut sdp = SessionDescription::new(
        Origin::new("rust-phone", session_id, session_id, local_ip.to_string()),
        "SIP Call Answer",
    );

    sdp.connection = Some(Connection::new_ip4(local_ip.to_string()));

    let mut media = Media::new_audio(local_rtp_port, vec![matched_format]);
    if matched_format == "0" {
        media = media.with_attribute(Attribute::new("rtpmap", Some("0 PCMU/8000")));
    } else {
        media = media.with_attribute(Attribute::new("rtpmap", Some("8 PCMA/8000")));
    }
    media = media.with_attribute(Attribute::flag(direction));

    sdp.medias.push(media);
    sdp
}

pub fn create_audio_hold_offer(
    local_ip: IpAddr,
    local_rtp_port: u16,
    session_id: u64,
    hold: bool,
) -> SessionDescription {
    let mut sdp = SessionDescription::new(
        Origin::new("rust-phone", session_id, session_id, local_ip.to_string()),
        "SIP Call",
    );

    sdp.connection = Some(Connection::new_ip4(local_ip.to_string()));

    let dir = if hold { "sendonly" } else { "sendrecv" };
    let media = Media::new_audio(local_rtp_port, vec!["0", "8"])
        .with_attribute(Attribute::new("rtpmap", Some("0 PCMU/8000")))
        .with_attribute(Attribute::new("rtpmap", Some("8 PCMA/8000")))
        .with_attribute(Attribute::flag(dir));

    sdp.medias.push(media);
    sdp
}

pub fn is_sdp_hold(sdp: &SessionDescription) -> bool {
    if let Some(c) = &sdp.connection
        && c.address == "0.0.0.0"
    {
        return true;
    }
    for m in &sdp.medias {
        if let Some(c) = &m.connection
            && c.address == "0.0.0.0"
        {
            return true;
        }
        for attr in &m.attributes {
            if attr.name == "sendonly" || attr.name == "inactive" {
                return true;
            }
        }
    }
    false
}

pub fn extract_media_socket_addr(sdp: &SessionDescription) -> Option<SocketAddr> {
    let media = sdp.medias.first()?;
    let port = media.port;

    let ip_str = sdp
        .connection
        .as_ref()
        .map(|c| c.address.clone())
        .or_else(|| media.connection.as_ref().map(|c| c.address.clone()))
        .unwrap_or_else(|| sdp.origin.unicast_address.clone());

    let ip: IpAddr = ip_str.parse().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    Some(SocketAddr::new(ip, port))
}
