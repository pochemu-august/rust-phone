/// Simple and fast linear resampler for telephony audio
pub fn resample_pcm(input: &[i16], in_rate: u32, out_rate: u32) -> Vec<i16> {
    if in_rate == out_rate || input.is_empty() {
        return input.to_vec();
    }

    let ratio = in_rate as f64 / out_rate as f64;
    let out_len = ((input.len() as f64) / ratio).round() as usize;
    let mut output = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let in_pos = i as f64 * ratio;
        let idx = in_pos.floor() as usize;
        let frac = (in_pos - idx as f64) as f32;

        if idx + 1 < input.len() {
            let s1 = input[idx] as f32;
            let s2 = input[idx + 1] as f32;
            let val = s1 + frac * (s2 - s1);
            output.push(val.clamp(i16::MIN as f32, i16::MAX as f32) as i16);
        } else if idx < input.len() {
            output.push(input[idx]);
        } else {
            output.push(0);
        }
    }

    output
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
}
