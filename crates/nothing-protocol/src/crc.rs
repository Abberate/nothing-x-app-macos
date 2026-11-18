/// CRC-16/MODBUS (poly 0xA001 reflected, init 0xFFFF), as used by the Nothing protocol.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= u16::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xA001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modbus_check_value() {
        assert_eq!(crc16(b"123456789"), 0x4B37);
    }

    #[test]
    fn battery_request_matches_capture() {
        // Captured from the reference implementation: "55 60 01 07 c0 00 00 01 ac df"
        let header = [0x55, 0x60, 0x01, 0x07, 0xC0, 0x00, 0x00, 0x01];
        assert_eq!(crc16(&header).to_le_bytes(), [0xAC, 0xDF]);
    }
}
