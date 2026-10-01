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
    notify: Arc<tokio::sync::Notify>,
    seq: u16,
    timestamp: u32,
    ssrc: u32,
}

impl RtpAudioStream {
    pub async fn bind(bind_addr: SocketAddr) -> Result<Self, std::io::Error> {
        let socket = UdpSocket::bind(bind_addr).await?;
        let local_addr = socket.local_addr()?;
        let socket = Arc::new(socket);

        let jitter_buffer = Arc::new(Mutex::new(JitterBuffer::new(3, 50)));
        let notify = Arc::new(tokio::sync::Notify::new());

        // Background receive task
        let recv_socket = Arc::clone(&socket);
        let recv_jb = Arc::clone(&jitter_buffer);
        let recv_notify = Arc::clone(&notify);
        tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];
            while let Ok((n, _from_peer)) = recv_socket.recv_from(&mut buf).await {
                if let Ok(pkt) = RtpPacket::parse(&buf[..n]) {
                    trace!("Received RTP seq {}", pkt.header.sequence_number);
                    recv_jb.lock().await.push(pkt);
                    recv_notify.notify_waiters();
                }
            }
        });

        let ssrc = rand_ssrc();

        Ok(Self {
            socket,
            local_addr,
            jitter_buffer,
            notify,
            seq: 1,
            timestamp: 0,
            ssrc,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn notify_handle(&self) -> Arc<tokio::sync::Notify> {
        Arc::clone(&self.notify)
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
        let mut jb = self.jitter_buffer.lock().await;
        while let Some(pkt) = jb.pop() {
            let pt = pkt.header.payload_type;
            if pt == 8 {
                return Some(AudioFrame::from_pcma(&pkt.payload, 8000));
            } else if pt == 0 {
                return Some(AudioFrame::from_pcmu(&pkt.payload, 8000));
            }
            // Discard non-audio packets (e.g. PT 13 Comfort Noise, PT 101 DTMF)
            trace!("Discarding non-audio RTP packet PT {}", pt);
        }
        None
    }
}

fn rand_ssrc() -> u32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    nanos ^ 0xa5a5a5a5
}
