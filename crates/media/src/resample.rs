/// Stateful continuous resampler for telephony audio avoiding clicks and phase drift
pub struct ContinuousResampler {
    pub in_rate: u32,
    pub out_rate: u32,
    pub phase: f64,
    last_sample: Option<i16>,
}

impl ContinuousResampler {
    pub fn new(in_rate: u32, out_rate: u32) -> Self {
        Self {
            in_rate,
            out_rate,
            phase: 0.0,
            last_sample: None,
        }
    }

    pub fn resample(&mut self, input: &[i16]) -> Vec<i16> {
        if self.in_rate == self.out_rate || input.is_empty() {
            return input.to_vec();
        }

        let ratio = self.in_rate as f64 / self.out_rate as f64;
        let mut output = Vec::new();

        if ratio <= 1.0 {
            // Upsampling: continuous linear interpolation across chunk boundaries
            let prev = self.last_sample.unwrap_or(input[0]);

            while self.phase < input.len() as f64 {
                let idx = self.phase.floor() as usize;
                let frac = (self.phase - idx as f64) as f32;

                let (s1, s2) = if idx == 0 {
                    (prev as f32, input[0] as f32)
                } else {
                    (input[idx - 1] as f32, input[idx] as f32)
                };

                let val = s1 + frac * (s2 - s1);
                output.push(val.clamp(i16::MIN as f32, i16::MAX as f32) as i16);
                self.phase += ratio;
            }

            self.last_sample = Some(input[input.len() - 1]);
        } else {
            // Downsampling: anti-aliasing boxcar averaging filter to prevent high-frequency aliasing
            let span = ratio.round().max(1.0) as usize;
            while self.phase < input.len() as f64 {
                let idx = self.phase.floor() as usize;
                let end_idx = (idx + span).min(input.len());
                let sum: i32 = input[idx..end_idx].iter().map(|&s| s as i32).sum();
                let count = (end_idx - idx).max(1) as i32;
                let val = (sum / count) as i16;
                output.push(val);
                self.phase += ratio;
            }
        }

        self.phase -= input.len() as f64;
        output
    }
}

/// Simple and fast linear resampler for telephony audio
pub fn resample_pcm(input: &[i16], in_rate: u32, out_rate: u32) -> Vec<i16> {
    if in_rate == out_rate || input.is_empty() {
        return input.to_vec();
    }

    let mut resampler = ContinuousResampler::new(in_rate, out_rate);
    resampler.resample(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resample_identity() {
        let samples = vec![100, 200, 300, 400];
        let out = resample_pcm(&samples, 8000, 8000);
        assert_eq!(samples, out);
    }

    #[test]
    fn test_resample_upsample() {
        let samples = vec![0, 1000, 2000];
        let out = resample_pcm(&samples, 8000, 16000);
        assert_eq!(out.len(), 6);
    }

    #[test]
    fn test_continuous_resampler_continuity() {
        let mut resampler = ContinuousResampler::new(8000, 16000);
        let chunk1 = vec![100, 200];
        let chunk2 = vec![300, 400];
        let out1 = resampler.resample(&chunk1);
        let out2 = resampler.resample(&chunk2);
        assert_eq!(out1.len(), 4);
        assert_eq!(out2.len(), 4);
    }
}
