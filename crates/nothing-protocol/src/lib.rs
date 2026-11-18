//! Nothing / CMF earbuds protocol, spoken over a Bluetooth SPP (RFCOMM) channel.
//!
//! This crate is pure (no I/O): it encodes requests, reassembles and decodes frames
//! coming back from the buds, and folds them into a [`state::DeviceState`].
//!
//! Wire format (little endian):
//! ```text
//! 55 | ctrl | 01 | cmd_lo cmd_hi | len_lo len_hi | op | payload… | crc_lo crc_hi
//! ```
//! `ctrl & 0x20` means a CRC-16/MODBUS trailer is present (always set by us: `0x60`).
//! Requests use `0xC0xx` (get) and `0xF0xx` (set). Responses clear bit 15
//! (`0x40xx` / `0x70xx`), pushes from the buds use `0xE0xx` / `0x60xx`.

pub mod cmd;
pub mod crc;
pub mod fastpair;
pub mod float;
pub mod frame;
pub mod gesture;
pub mod message;
pub mod model;
pub mod state;
pub mod types;

pub use frame::{Frame, Reassembler};
pub use message::{Event, Request};
pub use model::{Capabilities, ModelInfo};
pub use state::DeviceState;
pub use types::*;
