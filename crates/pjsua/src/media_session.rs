use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::warn;

use media::device::AudioIOHandle;
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
    playback_queue: Arc<std::sync::Mutex<Vec<i16>>>,
    audio_io: Option<AudioIOHandle>,
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
            playback_queue: Arc::new(std::sync::Mutex::new(Vec::new())),
            audio_io: None,
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

    pub fn start_hardware_audio(&mut self) {
        let stream_clone = Arc::clone(&self.stream);
        let remote_addr_opt = self.remote_addr;
        let pt = match self.codec {
            AudioCodecType::Pcmu => 0,
            AudioCodecType::Pcma => 8,
        };

        let queue = Arc::clone(&self.playback_queue);

        let on_mic = move |chunk: Vec<i16>| {
            if let Some(target) = remote_addr_opt {
                let frame = AudioFrame::new_mono(8000, chunk);
                let st = Arc::clone(&stream_clone);
                tokio::spawn(async move {
                    let _ = st.lock().await.send_frame(&frame, pt, target).await;
                });
            }
        };

        match media::device::start_audio_io(on_mic, queue) {
            Ok(handle) => {
                self.audio_io = Some(handle);

                let recv_stream = Arc::clone(&self.stream);
                let recv_queue = Arc::clone(&self.playback_queue);
                tokio::spawn(async move {
                    while let Some(frame) = recv_stream.lock().await.pop_frame().await {
                        if let Ok(mut lock) = recv_queue.lock() {
                            lock.extend_from_slice(&frame.samples);
                        }
                    }
                });
            }
            Err(e) => {
                warn!("Live audio hardware unavailable: {}", e);
            }
        }
    }

    pub fn mute_mic(&self, muted: bool) {
        if let Some(io) = &self.audio_io {
            io.set_mic_muted(muted);
        }
    }

    pub fn mute_speaker(&self, muted: bool) {
        if let Some(io) = &self.audio_io {
            io.set_speaker_muted(muted);
        }
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
