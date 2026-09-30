use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

use media::frame::AudioFrame;
use media::stream::RtpAudioStream;
use media::tone::generate_dtmf;

use crate::error::{PjsuaError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCodecType {
    Pcmu, // PT 0
    Pcma, // PT 8
}

pub struct MediaSession {
    stream: Arc<Mutex<RtpAudioStream>>,
    local_addr: SocketAddr,
    codec: AudioCodecType,
    remote_addr: Option<SocketAddr>,
}

impl MediaSession {
    pub async fn create(bind_addr: SocketAddr, codec: AudioCodecType) -> Result<Self> {
        let stream = RtpAudioStream::bind(bind_addr)
            .await
            .map_err(|e| PjsuaError::Media(format!("Failed to bind RTP stream: {}", e)))?;

        let local_addr = stream.local_addr();

        Ok(Self {
            stream: Arc::new(Mutex::new(stream)),
            local_addr,
            codec,
            remote_addr: None,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn set_remote_addr(&mut self, addr: SocketAddr) {
        self.remote_addr = Some(addr);
    }

    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.remote_addr
    }

    pub async fn send_pcm_samples(&self, samples: &[i16]) -> Result<()> {
        let remote_addr = self
            .remote_addr
            .ok_or_else(|| PjsuaError::Media("Remote RTP address not established".to_string()))?;

        let pt = match self.codec {
            AudioCodecType::Pcmu => 0,
            AudioCodecType::Pcma => 8,
        };

        let frame = AudioFrame::new_mono(8000, samples.to_vec());
        self.stream
            .lock()
            .await
            .send_frame(&frame, pt, remote_addr)
            .await
            .map_err(|e| PjsuaError::Media(format!("Failed to send RTP: {}", e)))?;

        Ok(())
    }

    pub async fn recv_pcm_samples(&self) -> Option<Vec<i16>> {
        let frame = self.stream.lock().await.pop_frame().await?;
        Some(frame.samples)
    }

    pub async fn send_dtmf(&self, digit: char) -> Result<()> {
        let remote_addr = self
            .remote_addr
            .ok_or_else(|| PjsuaError::Media("Remote RTP address not established".to_string()))?;

        let pt = match self.codec {
            AudioCodecType::Pcmu => 0,
            AudioCodecType::Pcma => 8,
        };

        if let Some(frame) = generate_dtmf(digit, 8000, 160) {
            self.stream
                .lock()
                .await
                .send_frame(&frame, pt, remote_addr)
                .await
                .map_err(|e| PjsuaError::Media(format!("Failed to send DTMF RTP: {}", e)))?;
        }
        Ok(())
    }
}
