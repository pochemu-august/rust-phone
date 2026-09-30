use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tracing::trace;

use crate::frame::AudioFrame;
use rtp::{JitterBuffer, RtpHeader, RtpPacket};

pub struct RtpAudioStream {
    socket: Arc<UdpSocket>,
    local_addr: SocketAddr,
    jitter_buffer: Arc<Mutex<JitterBuffer>>,
    seq: u16,
    timestamp: u32,
    ssrc: u32,
}

impl RtpAudioStream {
    pub async fn bind(bind_addr: SocketAddr) -> Result<Self, std::io::Error> {
        let socket = UdpSocket::bind(bind_addr).await?;
        let local_addr = socket.local_addr()?;
        let socket = Arc::new(socket);

        let jitter_buffer = Arc::new(Mutex::new(JitterBuffer::new(1, 50)));

        // Background receive task
        let recv_socket = Arc::clone(&socket);
        let recv_jb = Arc::clone(&jitter_buffer);
        tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];
            while let Ok((n, _from_peer)) = recv_socket.recv_from(&mut buf).await {
                if let Ok(pkt) = RtpPacket::parse(&buf[..n]) {
                    trace!("Received RTP seq {}", pkt.header.sequence_number);
                    recv_jb.lock().await.push(pkt);
                }
            }
        });

        let ssrc = rand_ssrc();

        Ok(Self {
            socket,
            local_addr,
            jitter_buffer,
            seq: 1,
            timestamp: 0,
            ssrc,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub async fn send_frame(
        &mut self,
        frame: &AudioFrame,
        payload_type: u8,
        target: SocketAddr,
    ) -> Result<(), std::io::Error> {
        let payload = if payload_type == 8 {
            frame.to_pcma()
        } else {
            frame.to_pcmu()
        };

        let header = RtpHeader::new(payload_type, self.seq, self.timestamp, self.ssrc);
        let pkt = RtpPacket::new(header, payload);
        let bytes = pkt.to_bytes();

        self.socket.send_to(&bytes, target).await?;

        self.seq = self.seq.wrapping_add(1);
        self.timestamp = self.timestamp.wrapping_add(frame.samples.len() as u32);

        Ok(())
    }

    pub async fn pop_frame(&self) -> Option<AudioFrame> {
        let pkt = self.jitter_buffer.lock().await.pop()?;
        let pt = pkt.header.payload_type;

        let frame = if pt == 8 {
            AudioFrame::from_pcma(&pkt.payload, 8000)
        } else {
            AudioFrame::from_pcmu(&pkt.payload, 8000)
        };

        Some(frame)
    }
}

fn rand_ssrc() -> u32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    nanos ^ 0xa5a5a5a5
}
