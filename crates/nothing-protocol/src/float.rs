//! EQ float encoding: IEEE-754 single precision, little endian on the wire, with one
//! quirk inherited from the firmware: a value whose three high (big-endian) bytes are
//! zero and whose lowest byte has bit 7 set is a negative tiny number with the sign
//! carried in that bit.

pub fn encode(value: f32) -> [u8; 4] {
    value.to_le_bytes()
}

pub fn decode(le: [u8; 4]) -> f32 {
    let mut be = [le[3], le[2], le[1], le[0]];
    if be[0] == 0 && be[1] == 0 && be[2] == 0 && be[3] & 0x80 != 0 {
        be[3] &= 0x7F;
        -f32::from_be_bytes(be)
    } else {
        f32::from_be_bytes(be)
    }
}

/// Reads a float at `offset` if the slice is long enough.
pub fn read(bytes: &[u8], offset: usize) -> Option<f32> {
    let b = bytes.get(offset..offset + 4)?;
    Some(decode([b[0], b[1], b[2], b[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        for v in [-6.0f32, -1.0, 0.0, 0.7, 3.0, 980.0, 6900.0] {
            assert_eq!(decode(encode(v)), v);
        }
    }
}
