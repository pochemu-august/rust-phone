pub mod frame;
pub mod g711;
pub mod stream;
pub mod tone;

pub use frame::AudioFrame;
pub use g711::{
    alaw_to_linear, linear_to_alaw, linear_to_ulaw, pcm16_to_pcma, pcm16_to_pcmu, pcma_to_pcm16,
    pcmu_to_pcm16, ulaw_to_linear,
};
pub use stream::RtpAudioStream;
pub use tone::{generate_dtmf, generate_sine};

pub mod prelude {
    pub use crate::frame::AudioFrame;
    pub use crate::g711::{
        alaw_to_linear, linear_to_alaw, linear_to_ulaw, pcm16_to_pcma, pcm16_to_pcmu,
        pcma_to_pcm16, pcmu_to_pcm16, ulaw_to_linear,
    };
    pub use crate::stream::RtpAudioStream;
    pub use crate::tone::{generate_dtmf, generate_sine};
}
