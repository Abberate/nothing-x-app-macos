use crate::crc::crc16;

pub const SYNC: u8 = 0x55;
pub const HEADER_LEN: usize = 8;
/// Control byte we send: bit 5 (`0x20`) = CRC trailer present.
pub const CTRL_WITH_CRC: u8 = 0x60;
const CRC_FLAG: u8 = 0x20;
pub const MAX_PAYLOAD: usize = 4096;
/// Hard cap on buffered bytes so a noisy link can never grow memory unbounded.
const MAX_BUFFER: usize = 64 * 1024;

/// One decoded protocol frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub command: u16,
    pub operation: u8,
    pub payload: Vec<u8>,
}

impl Frame {
    /// Serialises a request: header, payload, CRC-16 (little endian).
    pub fn encode(command: u16, operation: u8, payload: &[u8]) -> Vec<u8> {
        let len = payload.len().min(u16::MAX as usize) as u16;
        let mut out = Vec::with_capacity(HEADER_LEN + payload.len() + 2);
        out.extend_from_slice(&[SYNC, CTRL_WITH_CRC, 0x01]);
        out.extend_from_slice(&command.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.push(operation);
        out.extend_from_slice(&payload[..len as usize]);
        let crc = crc16(&out);
        out.extend_from_slice(&crc.to_le_bytes());
        out
    }
}

/// Buffers raw RFCOMM reads until whole frames are available. A single read can carry
/// several frames or only part of one; bytes before a sync byte and frames with a bad
/// CRC or an absurd length are skipped (resync one byte further).
#[derive(Debug, Default)]
pub struct Reassembler {
    buffer: Vec<u8>,
}

impl Reassembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn push(&mut self, chunk: &[u8]) -> Vec<Frame> {
        let mut frames = Vec::new();
        if chunk.is_empty() {
            return frames;
        }
        self.buffer.extend_from_slice(chunk);

        let buf = &self.buffer;
        let mut offset = 0usize;
        while offset < buf.len() {
            if buf[offset] != SYNC {
                offset += 1;
                continue;
            }
            if buf.len() - offset < HEADER_LEN {
                break;
            }
            let payload_len = u16::from_le_bytes([buf[offset + 5], buf[offset + 6]]) as usize;
            if payload_len > MAX_PAYLOAD {
                offset += 1;
                continue;
            }
            let crc_len = if buf[offset + 1] & CRC_FLAG != 0 {
                2
            } else {
                0
            };
            let total = HEADER_LEN + payload_len + crc_len;
            if buf.len() - offset < total {
                break;
            }
            let raw = &buf[offset..offset + total];
            if crc_len == 2 {
                let end = total - 2;
                let expected = u16::from_le_bytes([raw[end], raw[end + 1]]);
                // Requests carry a CRC over the whole frame; the buds' responses (seen on
                // CMF Buds Pro 2, fw 1.0.1.74) carry it over the payload only. Accept both.
                let whole = crc16(&raw[..end]);
                let payload_only = crc16(&raw[HEADER_LEN..end]);
                if expected != whole && expected != payload_only {
                    offset += 1;
                    continue;
                }
            }
            frames.push(Frame {
                command: u16::from_le_bytes([raw[3], raw[4]]),
                operation: raw[7],
                payload: raw[HEADER_LEN..HEADER_LEN + payload_len].to_vec(),
            });
            offset += total;
        }

        self.buffer.drain(..offset);
        if self.buffer.len() > MAX_BUFFER {
            self.buffer.clear();
        }
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_matches_reference_capture() {
        // From the reference implementation's log: battery read, op 1.
        assert_eq!(
            Frame::encode(0xC007, 1, &[]),
            vec![0x55, 0x60, 0x01, 0x07, 0xC0, 0x00, 0x00, 0x01, 0xAC, 0xDF]
        );
        assert_eq!(
            Frame::encode(0xC042, 2, &[]),
            vec![0x55, 0x60, 0x01, 0x42, 0xC0, 0x00, 0x00, 0x02, 0x21, 0x11]
        );
    }

    #[test]
    fn roundtrip_split_and_merged_chunks() {
        let a = Frame::encode(0x4007, 7, &[3, 2, 95, 3, 0x80 | 60, 4, 40]);
        let b = Frame::encode(0x7041, 9, &[]);
        let mut stream = vec![0x00, 0x12]; // garbage before sync
        stream.extend_from_slice(&a);
        stream.extend_from_slice(&b);

        let mut r = Reassembler::new();
        let mut got = Vec::new();
        for chunk in stream.chunks(3) {
            got.extend(r.push(chunk));
        }
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].command, 0x4007);
        assert_eq!(got[0].operation, 7);
        assert_eq!(got[0].payload, vec![3, 2, 95, 3, 0x80 | 60, 4, 40]);
        assert_eq!(got[1].command, 0x7041);

        let mut r = Reassembler::new();
        assert_eq!(r.push(&stream).len(), 2);
    }

    #[test]
    fn real_cmf_buds_pro_2_responses() {
        // Captured from a CMF Buds Pro 2 (firmware 1.0.1.74).
        let battery = [
            0x55, 0x60, 0x01, 0x07, 0x40, 0x05, 0x00, 0x01, 0x02, 0x02, 0x64, 0x03, 0x64, 0x1c,
            0xbc,
        ];
        let firmware = [
            0x55, 0x60, 0x01, 0x42, 0x40, 0x08, 0x00, 0x02, 0x31, 0x2e, 0x30, 0x2e, 0x31, 0x2e,
            0x37, 0x34, 0xd8, 0x35,
        ];
        let anc = [
            0x55, 0x60, 0x01, 0x1e, 0x40, 0x06, 0x00, 0x04, 0x02, 0x01, 0x00, 0x01, 0x07, 0x00,
            0x6f, 0xc9,
        ];
        let mut r = Reassembler::new();
        let mut stream = battery.to_vec();
        stream.extend_from_slice(&firmware);
        stream.extend_from_slice(&anc);
        let frames = r.push(&stream);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].command, 0x4007);
        assert_eq!(frames[0].payload, vec![2, 2, 100, 3, 100]);
        assert_eq!(frames[1].payload, b"1.0.1.74");
        assert_eq!(frames[2].payload, vec![2, 1, 0, 1, 7, 0]);
    }

    #[test]
    fn bad_crc_is_dropped_and_resynced() {
        let mut bad = Frame::encode(0x4007, 1, &[1, 2, 50]);
        let n = bad.len();
        bad[n - 1] ^= 0xFF;
        let good = Frame::encode(0x4042, 2, b"1.0.0");
        let mut r = Reassembler::new();
        let mut stream = bad;
        stream.extend_from_slice(&good);
        let frames = r.push(&stream);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].payload, b"1.0.0");
    }

    #[test]
    fn frame_without_crc_flag() {
        let mut raw = vec![0x55, 0x40, 0x01, 0x07, 0x40, 0x01, 0x00, 0x05, 0xAA];
        let mut r = Reassembler::new();
        let frames = r.push(&raw);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].payload, vec![0xAA]);
        raw.truncate(5);
        assert!(r.push(&raw).is_empty());
    }
}
