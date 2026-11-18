//! Google Fast Pair "message stream" (RFCOMM service `df21fe2c-2515-4fdb-8886-f12c4d67927c`).
//!
//! Right after the channel opens, the buds announce themselves:
//! `03 01 00 03 <model id: 3 bytes>` (group 3 = device information, code 1 = model id),
//! followed by the BLE address and session nonce. Each message is
//! `[group, code, len_hi, len_lo, data…]`.

#[derive(Debug, Default)]
pub struct MessageStream {
    buffer: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub group: u8,
    pub code: u8,
    pub data: Vec<u8>,
}

impl Message {
    pub fn model_id(&self) -> Option<u32> {
        (self.group == 0x03 && self.code == 0x01 && self.data.len() == 3)
            .then(|| u32::from_be_bytes([0, self.data[0], self.data[1], self.data[2]]))
    }
}

impl MessageStream {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Message> {
        self.buffer.extend_from_slice(chunk);
        let mut out = Vec::new();
        loop {
            if self.buffer.len() < 4 {
                break;
            }
            let len = u16::from_be_bytes([self.buffer[2], self.buffer[3]]) as usize;
            if len > 1024 {
                self.buffer.clear();
                break;
            }
            if self.buffer.len() < 4 + len {
                break;
            }
            let data = self.buffer[4..4 + len].to_vec();
            out.push(Message {
                group: self.buffer[0],
                code: self.buffer[1],
                data,
            });
            self.buffer.drain(..4 + len);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_capture() {
        // Captured from a CMF Buds Pro 2 (black, B187 revision).
        let mut s = MessageStream::default();
        let mut msgs = s.push(&[0x03, 0x01, 0x00, 0x03, 0x2f, 0x45, 0xf5]);
        msgs.extend(s.push(&[0x03, 0x02, 0x00, 0x06, 0x47, 0xc0, 0x43, 0x83, 0xf1, 0xb3]));
        msgs.extend(s.push(&[0x03, 0x03, 0x00, 0x03, 0x5a, 0x5a, 0xff]));
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].model_id(), Some(0x2F45F5));
        assert_eq!(msgs[1].model_id(), None);
    }
}
