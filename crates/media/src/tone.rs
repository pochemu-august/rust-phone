use crate::frame::AudioFrame;

pub fn generate_sine(
    frequency: f32,
    sample_rate: u32,
    duration_ms: u32,
    amplitude: f32,
) -> AudioFrame {
    let total_samples = (sample_rate * duration_ms / 1000) as usize;
    let mut samples = Vec::with_capacity(total_samples);

    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let val =
            (2.0 * std::f32::consts::PI * frequency * t).sin() * amplitude * (i16::MAX as f32);
        samples.push(val.clamp(i16::MIN as f32, i16::MAX as f32) as i16);
    }

    AudioFrame::new_mono(sample_rate, samples)
}

pub fn generate_dtmf(digit: char, sample_rate: u32, duration_ms: u32) -> Option<AudioFrame> {
    let (f_row, f_col) = match digit.to_ascii_uppercase() {
        '1' => (697.0, 1209.0),
        '2' => (697.0, 1336.0),
        '3' => (697.0, 1477.0),
        'A' => (697.0, 1633.0),
        '4' => (770.0, 1209.0),
        '5' => (770.0, 1336.0),
        '6' => (770.0, 1477.0),
        'B' => (770.0, 1633.0),
        '7' => (852.0, 1209.0),
        '8' => (852.0, 1336.0),
        '9' => (852.0, 1477.0),
        'C' => (852.0, 1633.0),
        '*' => (941.0, 1209.0),
        '0' => (941.0, 1336.0),
        '#' => (941.0, 1477.0),
        'D' => (941.0, 1633.0),
        _ => return None,
    };

    let total_samples = (sample_rate * duration_ms / 1000) as usize;
    let mut samples = Vec::with_capacity(total_samples);
    let amp = 0.45; // amplitude to avoid clipping when mixing two sine waves

    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let s1 = (2.0 * std::f32::consts::PI * f_row * t).sin();
        let s2 = (2.0 * std::f32::consts::PI * f_col * t).sin();
        let val = (s1 + s2) * amp * (i16::MAX as f32);
        samples.push(val.clamp(i16::MIN as f32, i16::MAX as f32) as i16);
    }

    Some(AudioFrame::new_mono(sample_rate, samples))
}
