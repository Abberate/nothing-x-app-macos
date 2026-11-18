//! Domain types and their wire encodings.

/// Noise-cancelling strength. Wire value of the "mode" byte when ANC is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AncStrength {
    High = 1,
    Mid = 2,
    Low = 3,
    Adaptive = 4,
}

impl AncStrength {
    pub const ALL: [AncStrength; 4] = [Self::High, Self::Mid, Self::Low, Self::Adaptive];

    pub fn from_wire(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::High),
            2 => Some(Self::Mid),
            3 => Some(Self::Low),
            4 => Some(Self::Adaptive),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::High => "HIGH",
            Self::Mid => "MID",
            Self::Low => "LOW",
            Self::Adaptive => "ADAPTIVE",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AncMode {
    Off,
    Transparency,
    NoiseCancelling(AncStrength),
}

impl AncMode {
    /// Value written with `SET_ANC` (`[0x01, value, 0x00]`).
    pub fn wire(self) -> u8 {
        match self {
            Self::Off => 0x05,
            Self::Transparency => 0x07,
            Self::NoiseCancelling(s) => s as u8,
        }
    }
}

/// Listening mode ("EQUALIZER" panel) for models using `GET/SET_LISTENING_MODE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EqPreset {
    DiracOpteo = 0,
    Rock = 1,
    Electronic = 2,
    Pop = 3,
    EnhanceVocals = 4,
    Classical = 5,
    Custom = 6,
}

impl EqPreset {
    /// Button order of the reference UI (two columns, then "Custom" full width).
    pub const UI_ORDER: [EqPreset; 7] = [
        Self::DiracOpteo,
        Self::Pop,
        Self::Rock,
        Self::Classical,
        Self::Electronic,
        Self::EnhanceVocals,
        Self::Custom,
    ];

    pub fn from_wire(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::DiracOpteo),
            1 => Some(Self::Rock),
            2 => Some(Self::Electronic),
            3 => Some(Self::Pop),
            4 => Some(Self::EnhanceVocals),
            5 => Some(Self::Classical),
            6 => Some(Self::Custom),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::DiracOpteo => "Dirac OPTEO",
            Self::Rock => "Rock",
            Self::Electronic => "Electronic",
            Self::Pop => "POP",
            Self::EnhanceVocals => "Enhance vocals",
            Self::Classical => "Classical",
            Self::Custom => "Custom",
        }
    }
}

/// Custom EQ gains in dB, as shown in the UI (Bass / Medium / Treble), range ±6.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CustomEq {
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
}

impl CustomEq {
    pub const MIN: f32 = -6.0;
    pub const MAX: f32 = 6.0;

    pub fn as_array(self) -> [f32; 3] {
        [self.bass, self.mid, self.treble]
    }

    pub fn from_array(v: [f32; 3]) -> Self {
        Self {
            bass: v[0],
            mid: v[1],
            treble: v[2],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    /// Device code used by battery, ring and gesture messages.
    pub fn code(self) -> u8 {
        match self {
            Self::Left => 0x02,
            Self::Right => 0x03,
        }
    }

    pub fn letter(self) -> &'static str {
        match self {
            Self::Left => "L",
            Self::Right => "R",
        }
    }
}

pub const DEVICE_LEFT: u8 = 0x02;
pub const DEVICE_RIGHT: u8 = 0x03;
pub const DEVICE_CASE: u8 = 0x04;
pub const DEVICE_SINGLE: u8 = 0x06;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BatteryLevel {
    pub percent: u8,
    pub charging: bool,
}

/// Ear tip fit result per bud (push `0xE00D`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EarFit {
    Good,
    Adjust,
    NotDetected,
    Unknown(u8),
}

impl EarFit {
    pub fn from_wire(v: u8) -> Self {
        match v {
            0 => Self::Good,
            1 => Self::Adjust,
            2 => Self::NotDetected,
            x => Self::Unknown(x),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Codec {
    Aac = 0,
    Lhdc = 1,
    Ldac = 2,
}

impl Codec {
    pub fn from_wire(v: u8) -> Self {
        match v {
            1 => Self::Lhdc,
            2 => Self::Ldac,
            _ => Self::Aac,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Aac => "AAC",
            Self::Lhdc => "LHDC",
            Self::Ldac => "LDAC",
        }
    }
}

/// Spatial audio modes; `capability_bit` is the bit in the model's `spatialAudio` mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpatialAudio {
    Off,
    HeadTracked,
    Fixed,
    Concert,
    Theatre,
    Game,
}

impl SpatialAudio {
    pub const ALL: [SpatialAudio; 6] = [
        Self::Off,
        Self::HeadTracked,
        Self::Fixed,
        Self::Concert,
        Self::Theatre,
        Self::Game,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::HeadTracked => "Head Tracked",
            Self::Fixed => "Fixed",
            Self::Concert => "Concert",
            Self::Theatre => "Theatre",
            Self::Game => "Game",
        }
    }

    pub fn capability_bit(self) -> Option<u8> {
        match self {
            Self::Off => None,
            Self::HeadTracked => Some(0x01),
            Self::Fixed => Some(0x02),
            Self::Concert => Some(0x08),
            Self::Theatre => Some(0x10),
            Self::Game => Some(0x20),
        }
    }

    /// `(mode, head_tracking)` bytes for `SET_SPATIAL_AUDIO`.
    pub fn wire(self) -> (u8, u8) {
        match self {
            Self::Off => (0, 0),
            Self::HeadTracked => (1, 1),
            Self::Fixed => (1, 0),
            Self::Concert => (2, 0),
            Self::Theatre => (3, 0),
            Self::Game => (4, 0),
        }
    }

    pub fn from_wire(mode: u8, head: u8) -> Self {
        if head == 1 {
            return Self::HeadTracked;
        }
        match mode {
            1 => Self::Fixed,
            2 => Self::Concert,
            3 => Self::Theatre,
            4 => Self::Game,
            _ => Self::Off,
        }
    }
}

/// A remembered host in the buds' dual-connection list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DualDevice {
    pub mac: [u8; 6],
    pub name: String,
    pub is_self: bool,
    pub connected: bool,
}

impl DualDevice {
    pub fn mac_string(&self) -> String {
        self.mac
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join("-")
    }
}
