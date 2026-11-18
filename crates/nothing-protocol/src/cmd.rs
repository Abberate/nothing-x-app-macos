//! Command identifiers.
//!
//! Requests are `0xC0xx` (read) and `0xF0xx` (write). The buds answer a read with the
//! same id with bit 14 cleared (`0x40xx`), acknowledge a write with `0x70xx` (same
//! operation id), and push unsolicited notifications as `0xE0xx` / `0x60xx`.
//! Handlers are keyed by `id | 0x8000` so a response and a push of the same kind share
//! one decoder (see [`key`]).

// ---- reads -------------------------------------------------------------------------
pub const GET_FIND_STATE: u16 = 0xC002;
pub const GET_SERIAL: u16 = 0xC006;
pub const GET_BATTERY: u16 = 0xC007;
pub const GET_IN_EAR: u16 = 0xC00E;
pub const GET_GESTURES: u16 = 0xC018;
pub const GET_ANC: u16 = 0xC01E;
pub const GET_EQ: u16 = 0xC01F;
pub const GET_PERSONALIZED_ANC: u16 = 0xC020;
pub const GET_DUAL_ENABLE: u16 = 0xC027;
pub const GET_DUAL_LIST: u16 = 0xC028;
pub const GET_CODEC: u16 = 0xC029;
pub const GET_LATENCY: u16 = 0xC041;
pub const GET_FIRMWARE: u16 = 0xC042;
pub const GET_CUSTOM_EQ: u16 = 0xC044;
pub const GET_ADVANCED_EQ_STATUS: u16 = 0xC04C;
pub const GET_ULTRA_BASS: u16 = 0xC04E;
pub const GET_SPATIAL_AUDIO: u16 = 0xC04F;
pub const GET_LISTENING_MODE: u16 = 0xC050;

// ---- writes ------------------------------------------------------------------------
pub const SET_RING: u16 = 0xF002;
pub const SET_GESTURE: u16 = 0xF003;
pub const SET_IN_EAR: u16 = 0xF004;
pub const SET_TIME: u16 = 0xF00A;
pub const SET_ANC: u16 = 0xF00F;
pub const SET_EQ: u16 = 0xF010;
pub const SET_EAR_FIT_TEST: u16 = 0xF014;
pub const SET_DUAL_ENABLE: u16 = 0xF01A;
pub const SET_DUAL_CONNECT: u16 = 0xF01B;
pub const SET_CODEC: u16 = 0xF01C;
pub const SET_LISTENING_MODE: u16 = 0xF01D;
pub const SET_LATENCY: u16 = 0xF040;
pub const SET_CUSTOM_EQ: u16 = 0xF041;
pub const SET_ULTRA_BASS: u16 = 0xF051;
pub const SET_SPATIAL_AUDIO: u16 = 0xF052;

// ---- pushes (keyed form) -----------------------------------------------------------
pub const PUSH_BATTERY: u16 = 0xE001;
pub const PUSH_ANC: u16 = 0xE003;
pub const PUSH_EAR_FIT: u16 = 0xE00D;
pub const PUSH_DUAL_DEVICE: u16 = 0xE00E;

/// Normalises a received command so responses (`0x40xx`) and reads (`0xC0xx`) map to
/// the same handler key, and pushes `0x60xx` map to `0xE0xx`.
pub const fn key(received: u16) -> u16 {
    received | 0x8000
}

/// `0xF0xx` writes are acknowledged one at a time (the reference app serialises them).
pub const fn is_set(command: u16) -> bool {
    command & 0xFF00 == 0xF000
}

/// A response has bit 15 clear: `0xF041` is acked by `0x7041`, `0xC007` by `0x4007`.
pub const fn is_response_to(received: u16, requested: u16) -> bool {
    received & 0x8000 == 0 && (received & 0x7FFF) == (requested & 0x7FFF)
}

/// Write acknowledgements (`0x7xxx`) carry no data worth decoding.
pub const fn is_write_ack(received: u16) -> bool {
    received & 0xF000 == 0x7000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ack_matching() {
        assert!(is_response_to(0x7041, SET_CUSTOM_EQ));
        assert!(is_response_to(0x4007, GET_BATTERY));
        assert!(!is_response_to(0xE001, GET_BATTERY));
        assert!(!is_response_to(0x700F, SET_CUSTOM_EQ));
        assert_eq!(key(0x4007), GET_BATTERY);
        assert_eq!(key(0x6001), PUSH_BATTERY);
        assert!(is_set(SET_ANC));
        assert!(!is_set(GET_ANC));
        assert!(!is_set(0xFC01));
    }
}
