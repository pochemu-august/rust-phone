use crate::error::{Result, RtpError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtpHeader {
    pub version: u8,
    pub padding: bool,
    pub extension: bool,
    pub marker: bool,
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub csrcs: Vec<u32>,
    pub extension_profile: Option<u16>,
    pub extension_data: Option<Vec<u8>>,
}

impl RtpHeader {
    pub fn new(payload_type: u8, seq: u16, timestamp: u32, ssrc: u32) -> Self {
        Self {
            version: 2,
            padding: false,
            extension: false,
            marker: false,
            payload_type,
            sequence_number: seq,
            timestamp,
            ssrc,
            csrcs: Vec::new(),
            extension_profile: None,
            extension_data: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtpPacket {
    pub header: RtpHeader,
    pub payload: Vec<u8>,
}

impl RtpPacket {
    pub fn new(header: RtpHeader, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            header,
            payload: payload.into(),
        }
    }

    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 12 {
            return Err(RtpError::PacketTooShort {
                expected: 12,
                got: data.len(),
            });
        }

        let b0 = data[0];
        let version = (b0 >> 6) & 0x03;
        if version != 2 {
            return Err(RtpError::InvalidVersion(version));
        }

        let padding = ((b0 >> 5) & 0x01) == 1;
        let extension = ((b0 >> 4) & 0x01) == 1;
        let csrc_count = (b0 & 0x0F) as usize;

        let b1 = data[1];
        let marker = ((b1 >> 7) & 0x01) == 1;
        let payload_type = b1 & 0x7F;

        let sequence_number = u16::from_be_bytes([data[2], data[3]]);
        let timestamp = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let ssrc = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

        let mut offset = 12;
        let expected_csrc_len = offset + csrc_count * 4;
        if data.len() < expected_csrc_len {
            return Err(RtpError::PacketTooShort {
                expected: expected_csrc_len,
                got: data.len(),
            });
        }

        let mut csrcs = Vec::with_capacity(csrc_count);
        for _ in 0..csrc_count {
            let csrc = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            csrcs.push(csrc);
            offset += 4;
        }

        let mut extension_profile = None;
        let mut extension_data = None;

        if extension {
            if data.len() < offset + 4 {
                return Err(RtpError::MalformedExtension);
            }
            let profile = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let ext_len_words = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
            let ext_len_bytes = ext_len_words * 4;
            offset += 4;

            if data.len() < offset + ext_len_bytes {
                return Err(RtpError::MalformedExtension);
            }

            extension_profile = Some(profile);
            extension_data = Some(data[offset..offset + ext_len_bytes].to_vec());
            offset += ext_len_bytes;
        }

        let mut payload_end = data.len();
        if padding {
            if payload_end <= offset {
                return Err(RtpError::InvalidPadding);
            }
            let pad_len = data[payload_end - 1] as usize;
            if pad_len == 0 || pad_len > (payload_end - offset) {
                return Err(RtpError::InvalidPadding);
            }
            payload_end -= pad_len;
        }

        let payload = data[offset..payload_end].to_vec();

        let header = RtpHeader {
            version,
            padding,
            extension,
            marker,
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
            csrcs,
            extension_profile,
            extension_data,
        };

        Ok(RtpPacket { header, payload })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        let v = (self.header.version & 0x03) << 6;
        let p = if self.header.padding { 1 << 5 } else { 0 };
        let x = if self.header.extension { 1 << 4 } else { 0 };
        let cc = (self.header.csrcs.len() as u8) & 0x0F;
        buf.push(v | p | x | cc);

        let m = if self.header.marker { 1 << 7 } else { 0 };
        let pt = self.header.payload_type & 0x7F;
        buf.push(m | pt);

        buf.extend_from_slice(&self.header.sequence_number.to_be_bytes());
        buf.extend_from_slice(&self.header.timestamp.to_be_bytes());
        buf.extend_from_slice(&self.header.ssrc.to_be_bytes());

        for csrc in &self.header.csrcs {
            buf.extend_from_slice(&csrc.to_be_bytes());
        }

        if let (Some(profile), Some(ext_bytes)) = (
            self.header.extension_profile,
            self.header.extension_data.as_ref(),
        ) {
            buf.extend_from_slice(&profile.to_be_bytes());
            let words = ext_bytes.len().div_ceil(4) as u16;
            buf.extend_from_slice(&words.to_be_bytes());
            buf.extend_from_slice(ext_bytes);
            let rem = (words as usize * 4) - ext_bytes.len();
            buf.resize(buf.len() + rem, 0);
        }

        buf.extend_from_slice(&self.payload);
        buf
    }
}
