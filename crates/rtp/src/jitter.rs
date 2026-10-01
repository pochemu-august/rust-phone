use crate::packet::RtpPacket;
use std::collections::BTreeMap;

/// Returns true if seq1 is strictly greater than seq2 considering u16 wrap-around
pub fn seq_greater(seq1: u16, seq2: u16) -> bool {
    seq1 != seq2 && (seq1.wrapping_sub(seq2) as i16) > 0
}

pub struct JitterBuffer {
    packets: BTreeMap<u16, RtpPacket>,
    next_seq: Option<u16>,
    last_ssrc: Option<u32>,
    prebuffer_count: usize,
    max_packets: usize,
}

impl JitterBuffer {
    pub fn new(prebuffer_count: usize, max_packets: usize) -> Self {
        Self {
            packets: BTreeMap::new(),
            next_seq: None,
            last_ssrc: None,
            prebuffer_count,
            max_packets,
        }
    }

    pub fn push(&mut self, packet: RtpPacket) {
        let ssrc = packet.header.ssrc;
        let seq = packet.header.sequence_number;

        // If SSRC changes (e.g. PBX switches between peer RTP and Music on Hold),
        // reset the jitter buffer to adapt to the new stream immediately.
        if let Some(prev_ssrc) = self.last_ssrc
            && prev_ssrc != ssrc
        {
            self.packets.clear();
            self.next_seq = None;
        }
        self.last_ssrc = Some(ssrc);

        if let Some(expected) = self.next_seq {
            let diff = seq.wrapping_sub(expected) as i16;
            // If delta is huge (sequence jump / stream reset), resync
            if diff.abs() > 1000 {
                self.packets.clear();
                self.next_seq = None;
            } else if diff < 0 {
                // Drop late packet
                return;
            }
        }

        self.packets.insert(seq, packet);

        // If buffer grows too large, drop oldest to prevent memory growth
        while self.packets.len() > self.max_packets {
            let oldest_key = *self.packets.keys().next().unwrap();
            self.packets.remove(&oldest_key);
            self.next_seq = Some(oldest_key.wrapping_add(1));
        }
    }

    pub fn pop(&mut self) -> Option<RtpPacket> {
        // If not initialized, wait until prebuffer count is reached
        if self.next_seq.is_none() {
            if self.packets.len() < self.prebuffer_count {
                return None;
            }
            let first_seq = *self.packets.keys().next()?;
            self.next_seq = Some(first_seq);
        }

        let expected = self.next_seq.unwrap();

        // Check if expected packet is available
        if let Some(pkt) = self.packets.remove(&expected) {
            self.next_seq = Some(expected.wrapping_add(1));
            return Some(pkt);
        }

        // If expected packet is missing, check if buffer is getting full (packet loss)
        if self.packets.len() >= self.prebuffer_count * 2 {
            // Skip the missing packet
            if let Some(&first_available) = self.packets.keys().next() {
                self.next_seq = Some(first_available.wrapping_add(1));
                return self.packets.remove(&first_available);
            }
        }

        None
    }

    pub fn len(&self) -> usize {
        self.packets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }
}
