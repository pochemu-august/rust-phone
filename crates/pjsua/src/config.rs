use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[derive(Debug, Clone)]
pub struct EndpointConfig {
    pub sip_bind_addr: SocketAddr,
    pub rtp_bind_ip: IpAddr,
    pub rtp_port_start: u16,
    pub user_agent: String,
}

impl Default for EndpointConfig {
    fn default() -> Self {
        Self {
            sip_bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 5060),
            rtp_bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            rtp_port_start: 4000,
            user_agent: "rust-phone/0.1.0 (PJSIP-Rust)".to_string(),
        }
    }
}
