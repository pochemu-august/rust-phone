use std::sync::Arc;
use tokio::sync::Mutex;

use sdp::SessionDescription;
use sip_ua::Dialog;

use crate::error::{PjsuaError, Result};
use crate::event::CallState;
use crate::media_session::MediaSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallDirection {
    Incoming,
    Outgoing,
}

pub struct Call {
    pub call_id: String,
    pub direction: CallDirection,
    pub local_uri: String,
    pub remote_uri: String,
    state: Arc<Mutex<CallState>>,
    dialog: Arc<Mutex<Option<Dialog>>>,
    media_session: Arc<Mutex<Option<MediaSession>>>,
    local_sdp: Arc<Mutex<Option<SessionDescription>>>,
    remote_sdp: Arc<Mutex<Option<SessionDescription>>>,
    local_hold: Arc<Mutex<bool>>,
    remote_hold: Arc<Mutex<bool>>,
}

impl Call {
    pub fn new(
        call_id: String,
        direction: CallDirection,
        local_uri: String,
        remote_uri: String,
        initial_state: CallState,
    ) -> Self {
        Self {
            call_id,
            direction,
            local_uri,
            remote_uri,
            state: Arc::new(Mutex::new(initial_state)),
            dialog: Arc::new(Mutex::new(None)),
            media_session: Arc::new(Mutex::new(None)),
            local_sdp: Arc::new(Mutex::new(None)),
            remote_sdp: Arc::new(Mutex::new(None)),
            local_hold: Arc::new(Mutex::new(false)),
            remote_hold: Arc::new(Mutex::new(false)),
        }
    }

    pub fn id(&self) -> &str {
        &self.call_id
    }

    pub async fn state(&self) -> CallState {
        *self.state.lock().await
    }

    pub async fn set_state(&self, new_state: CallState) {
        *self.state.lock().await = new_state;
    }

    pub async fn is_local_hold(&self) -> bool {
        *self.local_hold.lock().await
    }

    pub async fn set_local_hold(&self, hold: bool) {
        *self.local_hold.lock().await = hold;
    }

    pub async fn set_dialog(&self, dialog: Dialog) {
        *self.dialog.lock().await = Some(dialog);
    }

    pub async fn get_dialog(&self) -> Option<Dialog> {
        self.dialog.lock().await.clone()
    }

    pub async fn set_media_session(&self, session: MediaSession) {
        *self.media_session.lock().await = Some(session);
    }

    pub async fn set_remote_rtp_addr(&self, addr: std::net::SocketAddr) {
        let mut lock = self.media_session.lock().await;
        if let Some(ms) = lock.as_mut() {
            ms.set_remote_addr(addr);
        }
    }

    pub async fn set_local_sdp(&self, sdp: SessionDescription) {
        *self.local_sdp.lock().await = Some(sdp);
    }

    pub async fn get_local_sdp(&self) -> Option<SessionDescription> {
        self.local_sdp.lock().await.clone()
    }

    pub async fn set_remote_sdp(&self, sdp: SessionDescription) {
        *self.remote_sdp.lock().await = Some(sdp);
    }

    pub async fn get_remote_sdp(&self) -> Option<SessionDescription> {
        self.remote_sdp.lock().await.clone()
    }

    pub async fn send_audio(&self, samples: &[i16]) -> Result<()> {
        let lock = self.media_session.lock().await;
        if let Some(ms) = lock.as_ref() {
            ms.send_pcm_samples(samples).await
        } else {
            Err(PjsuaError::Media("No active media session".to_string()))
        }
    }

    pub async fn recv_audio(&self) -> Option<Vec<i16>> {
        let lock = self.media_session.lock().await;
        if let Some(ms) = lock.as_ref() {
            ms.recv_pcm_samples().await
        } else {
            None
        }
    }

    pub async fn send_dtmf(&self, digit: char) -> Result<()> {
        let lock = self.media_session.lock().await;
        if let Some(ms) = lock.as_ref() {
            ms.send_dtmf(digit).await
        } else {
            Err(PjsuaError::Media("No active media session".to_string()))
        }
    }

    pub async fn start_hardware_audio(&self) {
        let mut lock = self.media_session.lock().await;
        if let Some(ms) = lock.as_mut() {
            ms.start_hardware_audio().await;
        }
    }

    pub async fn flush_playback_queue(&self) {
        let lock = self.media_session.lock().await;
        if let Some(ms) = lock.as_ref() {
            ms.flush_playback_queue();
        }
    }

    pub async fn mute_mic(&self, muted: bool) {
        let lock = self.media_session.lock().await;
        if let Some(ms) = lock.as_ref() {
            ms.mute_mic(muted);
        }
    }

    pub async fn mute_speaker(&self, muted: bool) {
        let lock = self.media_session.lock().await;
        if let Some(ms) = lock.as_ref() {
            ms.mute_speaker(muted);
        }
    }

    pub async fn is_remote_hold(&self) -> bool {
        *self.remote_hold.lock().await
    }

    pub async fn set_remote_hold(&self, hold: bool) {
        *self.remote_hold.lock().await = hold;
    }

    /// Synchronizes hardware audio routing based on local vs remote hold states (matching MicroSIP logic):
    /// - Local Hold: Mute mic (stop transmitting) & mute speaker (local user hears no MoH)
    /// - Remote Hold: Mute mic, but unmute speaker so local user hears PBX Music On Hold
    /// - Active (No Hold): Unmute both mic and speaker
    pub async fn update_audio_hold_state(&self) {
        let local_hold = *self.local_hold.lock().await;
        let remote_hold = *self.remote_hold.lock().await;

        if local_hold {
            self.mute_mic(true).await;
            self.mute_speaker(true).await;
            self.flush_playback_queue().await;
        } else if remote_hold {
            self.mute_mic(true).await;
            self.mute_speaker(false).await;
        } else {
            self.mute_mic(false).await;
            self.mute_speaker(false).await;
            self.flush_playback_queue().await;
        }
    }
}
