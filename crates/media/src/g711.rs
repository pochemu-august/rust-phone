const SIGN_BIT: u8 = 0x80;
const QUANT_MASK: u8 = 0x0F;
const SEG_SHIFT: u8 = 4;
const SEG_MASK: u8 = 0x70;
const BIAS: i32 = 0x84;

const SEG_END: [i16; 8] = [0xFF, 0x1FF, 0x3FF, 0x7FF, 0xFFF, 0x1FFF, 0x3FFF, 0x7FFF];

fn search(val: i32, table: &[i16]) -> usize {
    for (i, &t) in table.iter().enumerate() {
        if val <= t as i32 {
            return i;
        }
    }
    table.len()
}

/// Convert 16-bit linear PCM to 8-bit A-law (PCMA)
pub fn linear_to_alaw(pcm_val: i16) -> u8 {
    let mut pcm_val = pcm_val as i32;
    let mask: u8;

    if pcm_val >= 0 {
        mask = 0xD5;
    } else {
        mask = 0x55;
        pcm_val = -pcm_val - 8;
        if pcm_val < 0 {
            pcm_val = 0;
        }
    }

    let seg = search(pcm_val, &SEG_END);

    if seg >= 8 {
        0x7F ^ mask
    } else {
        let mut aval = (seg as u8) << SEG_SHIFT;
        if seg < 2 {
            aval |= ((pcm_val >> 4) as u8) & QUANT_MASK;
        } else {
            aval |= ((pcm_val >> (seg + 3)) as u8) & QUANT_MASK;
        }
        aval ^ mask
    }
}

/// Convert 8-bit A-law (PCMA) to 16-bit linear PCM
pub fn alaw_to_linear(a_val: u8) -> i16 {
    let a_val = a_val ^ 0x55;

    let mut t = ((a_val & QUANT_MASK) as i32) << 4;
    let seg = ((a_val & SEG_MASK) >> SEG_SHIFT) as usize;

    match seg {
        0 => t += 8,
        1 => t += 0x108,
        _ => {
            t += 0x108;
            t <<= seg - 1;
        }
    }

    let result = if (a_val & SIGN_BIT) != 0 { t } else { -t };
    result.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

/// Convert 16-bit linear PCM to 8-bit mu-law (PCMU)
pub fn linear_to_ulaw(pcm_val: i16) -> u8 {
    let mut pcm_val = pcm_val as i32;
    let mask: u8;

    if pcm_val < 0 {
        pcm_val = BIAS - pcm_val;
        mask = 0x7F;
    } else {
        pcm_val += BIAS;
        mask = 0xFF;
    }

    let seg = search(pcm_val, &SEG_END);

    if seg >= 8 {
        0x7F ^ mask
    } else {
        let uval = ((seg as u8) << 4) | (((pcm_val >> (seg + 3)) as u8) & 0xF);
        uval ^ mask
    }
}

/// Convert 8-bit mu-law (PCMU) to 16-bit linear PCM
pub fn ulaw_to_linear(u_val: u8) -> i16 {
    if u_val == 0 {
        return 0;
    }

    let u_val = !u_val;
    let mut t = (((u_val & QUANT_MASK) as i32) << 3) + BIAS;
    t <<= ((u_val & SEG_MASK) >> SEG_SHIFT) as usize;

    let result = if (u_val & SIGN_BIT) != 0 {
        BIAS - t
    } else {
        t - BIAS
    };

    result.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

/// Batch convert 16-bit PCM buffer to PCMA (A-law)
pub fn pcm16_to_pcma(pcm: &[i16]) -> Vec<u8> {
    pcm.iter().map(|&s| linear_to_alaw(s)).collect()
}

/// Batch convert PCMA (A-law) buffer to 16-bit PCM
pub fn pcma_to_pcm16(pcma: &[u8]) -> Vec<i16> {
    pcma.iter().map(|&b| alaw_to_linear(b)).collect()
}

/// Batch convert 16-bit PCM buffer to PCMU (mu-law)
pub fn pcm16_to_pcmu(pcm: &[i16]) -> Vec<u8> {
    pcm.iter().map(|&s| linear_to_ulaw(s)).collect()
}

/// Batch convert PCMU (mu-law) buffer to 16-bit PCM
pub fn pcmu_to_pcm16(pcmu: &[u8]) -> Vec<i16> {
    pcmu.iter().map(|&b| ulaw_to_linear(b)).collect()
}
