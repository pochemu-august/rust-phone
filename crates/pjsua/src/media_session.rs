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
    remote_addr: Arc<std::sync::Mutex<Option<SocketAddr>>>,
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
            remote_addr: Arc::new(std::sync::Mutex::new(None)),
            playback_queue: Arc::new(std::sync::Mutex::new(Vec::new())),
            audio_io: None,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn set_remote_addr(&mut self, addr: SocketAddr) {
        if let Ok(mut lock) = self.remote_addr.lock() {
            *lock = Some(addr);
        }
    }

    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.remote_addr.lock().ok().and_then(|g| *g)
    }

    pub fn flush_playback_queue(&self) {
        if let Ok(mut lock) = self.playback_queue.lock() {
            lock.clear();
        }
    }

    pub async fn start_hardware_audio(&mut self) {
        if self.audio_io.is_some() {
            return;
        }

        let (mic_tx, mut mic_rx) = tokio::sync::mpsc::channel::<Vec<i16>>(64);

        let on_mic = move |chunk: Vec<i16>| {
            let _ = mic_tx.try_send(chunk);
        };

        let stream_clone = Arc::clone(&self.stream);
        let remote_addr_ref = Arc::clone(&self.remote_addr);
        let pt = match self.codec {
            AudioCodecType::Pcmu => 0,
            AudioCodecType::Pcma => 8,
        };

        tokio::spawn(async move {
            while let Some(chunk) = mic_rx.recv().await {
                let target_opt = remote_addr_ref.lock().ok().and_then(|g| *g);
                if let Some(target) = target_opt {
                    let frame = AudioFrame::new_mono(8000, chunk);
                    let _ = stream_clone
                        .lock()
                        .await
                        .send_frame(&frame, pt, target)
                        .await;
                }
            }
        });

        let queue = Arc::clone(&self.playback_queue);
        match media::device::start_audio_io(on_mic, queue) {
            Ok(handle) => {
                self.audio_io = Some(handle);

                let recv_stream = Arc::clone(&self.stream);
                let recv_queue = Arc::clone(&self.playback_queue);
                let notify = self.stream.lock().await.notify_handle();
                tokio::spawn(async move {
                    loop {
                        let maybe_frame = {
                            let stream = recv_stream.lock().await;
                            stream.pop_frame().await
                        };
                        if let Some(frame) = maybe_frame {
                            if let Ok(mut lock) = recv_queue.lock() {
                                lock.extend_from_slice(&frame.samples);
                            }
                        } else {
                            notify.notified().await;
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
            .remote_addr()
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
            .remote_addr()
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
