//! Bluetooth Classic RFCOMM transport for Nothing / CMF earbuds on macOS.
//!
//! On current macOS releases IOBluetooth is a shim over CoreBluetooth, which delivers
//! its callbacks on the **main dispatch queue**. Every IOBluetooth call is therefore
//! marshalled to the main thread; the main thread must be running a run loop (AppKit
//! in the GUI, [`run_main_loop`] in a CLI). Results come back through a callback
//! ([`LinkSink`]) so the protocol logic can live on any thread.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

/// Nothing's proprietary SPP service (control protocol).
pub const SPP_UUID: [u8; 16] = uuid("aeac4a03-dff5-498f-843a-34487cf133eb");
/// Google Fast Pair message stream service (announces the model id).
pub const FASTPAIR_UUID: [u8; 16] = uuid("df21fe2c-2515-4fdb-8886-f12c4d67927c");

/// A device paired with this Mac.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedDevice {
    pub name: String,
    /// `xx-xx-xx-xx-xx-xx` as reported by IOBluetooth.
    pub address: String,
    pub connected: bool,
    pub class_of_device: u32,
}

impl PairedDevice {
    /// Heuristic used to pre-select candidates (Nothing and CMF products).
    pub fn looks_like_nothing(&self) -> bool {
        let n = self.name.to_lowercase();
        n.contains("nothing")
            || n.starts_with("cmf")
            || n.starts_with("ear (")
            || n.starts_with("ear(")
    }
}

/// Which RFCOMM service to open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    Spp,
    FastPair,
    Channel(u8),
}

impl Service {
    pub fn uuid(self) -> Option<[u8; 16]> {
        match self {
            Self::Spp => Some(SPP_UUID),
            Self::FastPair => Some(FASTPAIR_UUID),
            Self::Channel(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkEvent {
    Opened { channel: u8 },
    Data(Vec<u8>),
    Closed,
    Failed(String),
}

/// Receives link events; called on the main thread, so it must be quick (e.g. a channel send).
pub type LinkSink = Box<dyn Fn(LinkEvent) + Send + 'static>;

const fn hex(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("invalid hex digit"),
    }
}

const fn uuid(s: &str) -> [u8; 16] {
    let b = s.as_bytes();
    let mut out = [0u8; 16];
    let (mut i, mut o) = (0, 0);
    while i < b.len() {
        if b[i] == b'-' {
            i += 1;
            continue;
        }
        out[o] = (hex(b[i]) << 4) | hex(b[i + 1]);
        o += 1;
        i += 2;
    }
    out
}

/// Normalises `AA:BB:CC:DD:EE:FF` / `aa-bb-cc-dd-ee-ff` to `aa-bb-cc-dd-ee-ff`.
pub fn normalize_address(addr: &str) -> String {
    addr.trim().replace(':', "-").to_lowercase()
}
