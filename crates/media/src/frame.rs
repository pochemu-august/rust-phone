use crate::g711::{pcm16_to_pcma, pcm16_to_pcmu, pcma_to_pcm16, pcmu_to_pcm16};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFrame {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<i16>,
}

impl AudioFrame {
    pub fn new_mono(sample_rate: u32, samples: Vec<i16>) -> Self {
        Self {
            sample_rate,
            channels: 1,
            samples,
        }
    }

    pub fn silence(sample_rate: u32, duration_ms: u32) -> Self {
        let count = (sample_rate * duration_ms / 1000) as usize;
        Self {
            sample_rate,
            channels: 1,
            samples: vec![0; count],
        }
    }

    pub fn duration_ms(&self) -> u32 {
        if self.sample_rate == 0 || self.channels == 0 {
            return 0;
        }
        ((self.samples.len() as u64 * 1000) / (self.sample_rate as u64 * self.channels as u64))
            as u32
    }

    pub fn to_pcma(&self) -> Vec<u8> {
        pcm16_to_pcma(&self.samples)
    }

    pub fn to_pcmu(&self) -> Vec<u8> {
        pcm16_to_pcmu(&self.samples)
    }

    pub fn from_pcma(data: &[u8], sample_rate: u32) -> Self {
        let samples = pcma_to_pcm16(data);
        Self::new_mono(sample_rate, samples)
    }

    pub fn from_pcmu(data: &[u8], sample_rate: u32) -> Self {
        let samples = pcmu_to_pcm16(data);
        Self::new_mono(sample_rate, samples)
    }
}
