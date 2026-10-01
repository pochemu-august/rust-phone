use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tracing::{error, info};

use crate::resample::ContinuousResampler;

#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_default: bool,
}

pub fn list_audio_devices() -> (Vec<AudioDeviceInfo>, Vec<AudioDeviceInfo>) {
    let host = cpal::default_host();

    let mut input_devs = Vec::new();
    let default_in = host.default_input_device().and_then(|d| d.name().ok());
    if let Ok(devices) = host.input_devices() {
        for d in devices {
            if let Ok(name) = d.name() {
                let is_default = default_in.as_ref() == Some(&name);
                input_devs.push(AudioDeviceInfo { name, is_default });
            }
        }
    }

    let mut output_devs = Vec::new();
    let default_out = host.default_output_device().and_then(|d| d.name().ok());
    if let Ok(devices) = host.output_devices() {
        for d in devices {
            if let Ok(name) = d.name() {
                let is_default = default_out.as_ref() == Some(&name);
                output_devs.push(AudioDeviceInfo { name, is_default });
            }
        }
    }

    (input_devs, output_devs)
}

pub struct AudioIOHandle {
    _input_stream: Option<cpal::Stream>,
    _output_stream: Option<cpal::Stream>,
    pub is_mic_muted: Arc<AtomicBool>,
    pub is_speaker_muted: Arc<AtomicBool>,
}

impl AudioIOHandle {
    pub fn set_mic_muted(&self, muted: bool) {
        self.is_mic_muted.store(muted, Ordering::SeqCst);
    }

    pub fn set_speaker_muted(&self, muted: bool) {
        self.is_speaker_muted.store(muted, Ordering::SeqCst);
    }
}

// Safety: cpal::Stream can be safely transferred between threads and dropped
unsafe impl Send for AudioIOHandle {}
unsafe impl Sync for AudioIOHandle {}

/// Starts bidirectional audio I/O connecting microphone capture and speaker playback
pub fn start_audio_io<F>(
    on_mic_pcm_chunk: F,
    playback_queue: Arc<Mutex<Vec<i16>>>,
) -> Result<AudioIOHandle, String>
where
    F: Fn(Vec<i16>) + Send + Sync + 'static,
{
    let host = cpal::default_host();
    let is_mic_muted = Arc::new(AtomicBool::new(false));
    let is_speaker_muted = Arc::new(AtomicBool::new(false));

    // 1. Microphone capture setup
    let input_device = host
        .default_input_device()
        .ok_or_else(|| "No default audio input device (microphone) found".to_string())?;

    let in_config = input_device
        .default_input_config()
        .map_err(|e| format!("Failed to get default input config: {}", e))?;

    let in_sample_rate = in_config.sample_rate().0;
    let in_channels = in_config.channels() as usize;

    let mic_callback = Arc::new(on_mic_pcm_chunk);
    let mic_buffer = Arc::new(Mutex::new(Vec::<i16>::new()));
    let mic_muted_clone = Arc::clone(&is_mic_muted);

    let err_fn = |err| error!("Audio stream error: {}", err);

    let mic_buf_clone = Arc::clone(&mic_buffer);
    let mic_cb_clone = Arc::clone(&mic_callback);

    let mut in_resampler = ContinuousResampler::new(in_sample_rate, 8000);

    let input_stream = input_device
        .build_input_stream(
            &in_config.into(),
            move |data: &[f32], _: &_| {
                if mic_muted_clone.load(Ordering::Relaxed) {
                    return;
                }

                // Downmix channels to mono if needed
                let mut mono_pcm = Vec::with_capacity(data.len() / in_channels.max(1));
                for chunk in data.chunks_exact(in_channels) {
                    let mono_sample: f32 = chunk.iter().sum::<f32>() / (in_channels as f32);
                    let s16 = (mono_sample * (i16::MAX as f32))
                        .clamp(i16::MIN as f32, i16::MAX as f32)
                        as i16;
                    mono_pcm.push(s16);
                }

                // Continuously resample to 8000 Hz telephony rate with phase preservation
                let resampled_8k = in_resampler.resample(&mono_pcm);

                let mut lock = mic_buf_clone.lock().unwrap();
                lock.extend_from_slice(&resampled_8k);

                // When 160 samples (20ms) are ready, emit chunk
                while lock.len() >= 160 {
                    let chunk: Vec<i16> = lock.drain(0..160).collect();
                    mic_cb_clone(chunk);
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| format!("Failed to build input stream: {}", e))?;

    input_stream
        .play()
        .map_err(|e| format!("Failed to start input stream: {}", e))?;

    // 2. Speaker playback setup
    let output_device = host
        .default_output_device()
        .ok_or_else(|| "No default audio output device (speaker) found".to_string())?;

    let out_config = output_device
        .default_output_config()
        .map_err(|e| format!("Failed to get default output config: {}", e))?;

    let out_sample_rate = out_config.sample_rate().0;
    let out_channels = out_config.channels() as usize;
    let speaker_muted_clone = Arc::clone(&is_speaker_muted);

    let play_queue_clone = Arc::clone(&playback_queue);

    let mut out_resampler = ContinuousResampler::new(8000, out_sample_rate);
    let mut is_playing = false;
    let prebuffer_threshold = 320; // 40ms of 8kHz telephony audio
    let mut last_sample_f32 = 0.0f32;
    let mut out_buffer: Vec<f32> = Vec::with_capacity(4096);

    let output_stream = output_device
        .build_output_stream(
            &out_config.into(),
            move |data: &mut [f32], _: &_| {
                if speaker_muted_clone.load(Ordering::Relaxed) {
                    data.fill(0.0);
                    last_sample_f32 = 0.0;
                    out_buffer.clear();
                    return;
                }

                let frames_needed = data.len() / out_channels.max(1);

                // Initial prebuffer cushion before first playback starts
                if !is_playing {
                    let lock = play_queue_clone.lock().unwrap();
                    if lock.len() >= prebuffer_threshold {
                        is_playing = true;
                    } else {
                        data.fill(0.0);
                        return;
                    }
                }

                // If out_buffer doesn't have enough samples for this frame, pull & resample from play_queue
                while out_buffer.len() < frames_needed {
                    let mut lock = play_queue_clone.lock().unwrap();
                    if lock.is_empty() {
                        break;
                    }
                    let chunk_size = 160.min(lock.len());
                    let chunk: Vec<i16> = lock.drain(0..chunk_size).collect();
                    drop(lock);

                    let upsampled = out_resampler.resample(&chunk);
                    out_buffer.extend(upsampled.iter().map(|&s| (s as f32) / (i16::MAX as f32)));
                }

                // Output samples from out_buffer to all channels without dropping any samples
                let available = out_buffer.len().min(frames_needed);
                for (idx, frame) in data.chunks_mut(out_channels).enumerate() {
                    let sample_f32 = if idx < available {
                        last_sample_f32 = out_buffer[idx];
                        out_buffer[idx]
                    } else {
                        // Smooth decay concealment if buffer underrun occurs
                        last_sample_f32 *= 0.95;
                        last_sample_f32
                    };

                    for ch in frame.iter_mut() {
                        *ch = sample_f32;
                    }
                }

                // Drain the samples we just played; any remainder stays for next callback!
                out_buffer.drain(0..available);
            },
            err_fn,
            None,
        )
        .map_err(|e| format!("Failed to build output stream: {}", e))?;

    output_stream
        .play()
        .map_err(|e| format!("Failed to start output stream: {}", e))?;

    info!(
        "Audio hardware active: In={}Hz ({}ch), Out={}Hz ({}ch)",
        in_sample_rate, in_channels, out_sample_rate, out_channels
    );

    Ok(AudioIOHandle {
        _input_stream: Some(input_stream),
        _output_stream: Some(output_stream),
        is_mic_muted,
        is_speaker_muted,
    })
}
