//! Touch / smart-dial gesture definitions for the CMF Buds Pro 2 family.
//!
//! Read with `GET_GESTURES`: `[count, (device, common, kind, action)*]`.
//! Written with `SET_GESTURE`: `[0x01, device, common, kind, action]`.

use crate::types::{DEVICE_CASE, DEVICE_LEFT, DEVICE_RIGHT};

/// Gesture kinds (`kind` byte).
pub mod kind {
    pub const SINGLE: u8 = 1;
    pub const DOUBLE: u8 = 2;
    pub const TRIPLE: u8 = 3;
    pub const HOLD: u8 = 7;
    pub const DOUBLE_HOLD: u8 = 9;
    pub const ROTATE: u8 = 10;
}

/// Action codes (`action` byte).
pub mod action {
    pub const NONE: u8 = 1;
    pub const PLAY_PAUSE: u8 = 2;
    pub const ANSWER_CALL: u8 = 3;
    pub const SKIP_BACK: u8 = 8;
    pub const SKIP_FORWARD: u8 = 9;
    /// Noise control cycling through Transparency, Noise cancellation and Off.
    pub const NOISE_CONTROL_ALL: u8 = 10;
    pub const VOICE_ASSISTANT: u8 = 11;
    pub const LOW_LAG_TOGGLE: u8 = 17;
    pub const VOLUME_UP: u8 = 18;
    pub const VOLUME_DOWN: u8 = 19;
    /// Noise cancellation + Off.
    pub const NOISE_CONTROL_NC_OFF: u8 = 20;
    /// Transparency + Off.
    pub const NOISE_CONTROL_TRANSPARENCY_OFF: u8 = 21;
    /// Transparency + Noise cancellation.
    pub const NOISE_CONTROL_TRANSPARENCY_NC: u8 = 22;
    pub const VOLUME_CONTROL: u8 = 23;
    pub const ANSWER_CALL_MUTE: u8 = 25;
    pub const HANG_UP_DECLINE: u8 = 26;
}

/// Which modes a "Noise control" gesture cycles through, in UI order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AncCycle {
    pub transparency: bool,
    pub noise_cancellation: bool,
    pub off: bool,
}

impl Default for AncCycle {
    fn default() -> Self {
        Self {
            transparency: true,
            noise_cancellation: true,
            off: false,
        }
    }
}

impl AncCycle {
    pub fn from_action(action: u8) -> Option<Self> {
        let (transparency, noise_cancellation, off) = match action {
            action::NOISE_CONTROL_ALL => (true, true, true),
            action::NOISE_CONTROL_NC_OFF => (false, true, true),
            action::NOISE_CONTROL_TRANSPARENCY_OFF => (true, false, true),
            action::NOISE_CONTROL_TRANSPARENCY_NC => (true, true, false),
            _ => return None,
        };
        Some(Self {
            transparency,
            noise_cancellation,
            off,
        })
    }

    /// `None` when fewer than two modes are selected (not allowed by the buds).
    pub fn to_action(self) -> Option<u8> {
        match (self.transparency, self.noise_cancellation, self.off) {
            (true, true, true) => Some(action::NOISE_CONTROL_ALL),
            (false, true, true) => Some(action::NOISE_CONTROL_NC_OFF),
            (true, false, true) => Some(action::NOISE_CONTROL_TRANSPARENCY_OFF),
            (true, true, false) => Some(action::NOISE_CONTROL_TRANSPARENCY_NC),
            _ => None,
        }
    }

    pub fn count(self) -> usize {
        [self.transparency, self.noise_cancellation, self.off]
            .iter()
            .filter(|b| **b)
            .count()
    }
}

pub fn is_noise_control(action: u8) -> bool {
    AncCycle::from_action(action).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GestureOption {
    /// Representative action; for "Noise control" the real code depends on [`AncCycle`].
    pub action: u8,
    pub label: &'static str,
}

const fn opt(action: u8, label: &'static str) -> GestureOption {
    GestureOption { action, label }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Buds,
    Case,
}

/// One configurable gesture row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GestureSlot {
    pub id: &'static str,
    pub title: &'static str,
    pub target: Target,
    pub kind: u8,
    /// `common`/button byte; `None` = match any when reading (earbud slots).
    pub common: Option<u8>,
    pub options: &'static [GestureOption],
    /// Fixed, non-customisable behaviour shown under the current value.
    pub suffix: Option<&'static str>,
    pub anc_toggle: bool,
}

impl GestureSlot {
    pub fn send_common(&self) -> u8 {
        self.common.unwrap_or(1)
    }

    pub fn matches(&self, kind: u8, common: u8) -> bool {
        self.kind == kind && self.common.is_none_or(|c| c == common)
    }

    pub fn label_for(&self, action: u8) -> Option<&'static str> {
        if self.anc_toggle && is_noise_control(action) {
            return Some("Noise control");
        }
        self.options
            .iter()
            .find(|o| o.action == action)
            .map(|o| o.label)
    }
}

const BUD_DOUBLE: &[GestureOption] = &[
    opt(action::PLAY_PAUSE, "Play/Pause"),
    opt(action::SKIP_BACK, "Skip Back"),
    opt(action::SKIP_FORWARD, "Skip Forward"),
    opt(action::VOICE_ASSISTANT, "Voice Assistant"),
    opt(action::NONE, "No action"),
];
const BUD_TRIPLE: &[GestureOption] = &[
    opt(action::SKIP_BACK, "Skip Back"),
    opt(action::SKIP_FORWARD, "Skip Forward"),
    opt(action::VOICE_ASSISTANT, "Voice Assistant"),
    opt(action::NONE, "No action"),
];
const BUD_HOLD: &[GestureOption] = &[
    opt(action::NOISE_CONTROL_ALL, "Noise control"),
    opt(action::VOICE_ASSISTANT, "Voice Assistant"),
    opt(action::NONE, "No action"),
];
const BUD_DOUBLE_HOLD: &[GestureOption] = &[
    opt(action::VOLUME_UP, "Volume up"),
    opt(action::VOLUME_DOWN, "Volume down"),
    opt(action::VOICE_ASSISTANT, "Voice Assistant"),
    opt(action::NONE, "No action"),
];

const CASE_PRESS: &[GestureOption] = &[
    opt(action::PLAY_PAUSE, "Play/Pause"),
    opt(action::SKIP_FORWARD, "Skip Forward"),
    opt(action::SKIP_BACK, "Skip Back"),
    opt(action::VOICE_ASSISTANT, "Voice Assistant"),
    opt(action::LOW_LAG_TOGGLE, "Low lag mode on/off"),
    opt(action::NONE, "No action"),
];
const CASE_DOUBLE_CALL: &[GestureOption] = &[
    opt(action::ANSWER_CALL, "Answer calls"),
    opt(
        action::ANSWER_CALL_MUTE,
        "Answer calls / During calls: Mute / unmute Mic",
    ),
    opt(action::NONE, "No action"),
];
const CASE_TRIPLE_CALL: &[GestureOption] = &[
    opt(
        action::HANG_UP_DECLINE,
        "Hang up calls / Decline incoming calls",
    ),
    opt(action::NONE, "No action"),
];
const CASE_HOLD: &[GestureOption] = &[
    opt(action::NOISE_CONTROL_ALL, "Noise control"),
    opt(action::VOICE_ASSISTANT, "Voice Assistant"),
    opt(action::LOW_LAG_TOGGLE, "Low lag mode on/off"),
    opt(action::NONE, "No action"),
];
const CASE_ROTATE: &[GestureOption] = &[
    opt(action::VOLUME_CONTROL, "Volume control"),
    opt(action::NONE, "No action"),
];

/// Earbud gestures, in UI order.
pub const BUD_SLOTS: &[GestureSlot] = &[
    GestureSlot {
        id: "double_tap",
        title: "DOUBLE TAP",
        target: Target::Buds,
        kind: kind::DOUBLE,
        common: None,
        options: BUD_DOUBLE,
        suffix: Some("Answer calls"),
        anc_toggle: false,
    },
    GestureSlot {
        id: "triple_tap",
        title: "TRIPLE TAP",
        target: Target::Buds,
        kind: kind::TRIPLE,
        common: None,
        options: BUD_TRIPLE,
        suffix: Some("Hang up calls / Decline Incoming call"),
        anc_toggle: false,
    },
    GestureSlot {
        id: "tap_and_hold",
        title: "TAP & HOLD",
        target: Target::Buds,
        kind: kind::HOLD,
        common: None,
        options: BUD_HOLD,
        suffix: None,
        anc_toggle: true,
    },
    GestureSlot {
        id: "double_tap_and_hold",
        title: "DOUBLE TAP & HOLD",
        target: Target::Buds,
        kind: kind::DOUBLE_HOLD,
        common: None,
        options: BUD_DOUBLE_HOLD,
        suffix: None,
        anc_toggle: false,
    },
];

/// Smart dial (case) gestures.
pub const CASE_SINGLE: GestureSlot = GestureSlot {
    id: "case_single_press",
    title: "SINGLE PRESS",
    target: Target::Case,
    kind: kind::SINGLE,
    common: Some(1),
    options: CASE_PRESS,
    suffix: None,
    anc_toggle: false,
};
pub const CASE_DOUBLE: GestureSlot = GestureSlot {
    id: "case_double_press",
    title: "DOUBLE PRESS",
    target: Target::Case,
    kind: kind::DOUBLE,
    common: Some(1),
    options: CASE_PRESS,
    suffix: None,
    anc_toggle: false,
};
pub const CASE_DOUBLE_CALL_SLOT: GestureSlot = GestureSlot {
    id: "case_double_press_call",
    title: "DOUBLE PRESS · CALLS",
    target: Target::Case,
    kind: kind::DOUBLE,
    common: Some(9),
    options: CASE_DOUBLE_CALL,
    suffix: None,
    anc_toggle: false,
};
pub const CASE_TRIPLE: GestureSlot = GestureSlot {
    id: "case_triple_press",
    title: "TRIPLE PRESS",
    target: Target::Case,
    kind: kind::TRIPLE,
    common: Some(1),
    options: CASE_PRESS,
    suffix: None,
    anc_toggle: false,
};
pub const CASE_TRIPLE_CALL_SLOT: GestureSlot = GestureSlot {
    id: "case_triple_press_call",
    title: "TRIPLE PRESS · CALLS",
    target: Target::Case,
    kind: kind::TRIPLE,
    common: Some(9),
    options: CASE_TRIPLE_CALL,
    suffix: None,
    anc_toggle: false,
};
pub const CASE_HOLD_SLOT: GestureSlot = GestureSlot {
    id: "case_press_hold",
    title: "PRESS & HOLD",
    target: Target::Case,
    kind: kind::HOLD,
    common: Some(1),
    options: CASE_HOLD,
    suffix: None,
    anc_toggle: true,
};
pub const CASE_ROTATE_SLOT: GestureSlot = GestureSlot {
    id: "case_rotate",
    title: "ROTATE",
    target: Target::Case,
    kind: kind::ROTATE,
    common: Some(1),
    options: CASE_ROTATE,
    suffix: None,
    anc_toggle: false,
};

pub const CASE_SLOTS: &[GestureSlot] = &[
    CASE_SINGLE,
    CASE_DOUBLE,
    CASE_DOUBLE_CALL_SLOT,
    CASE_TRIPLE,
    CASE_TRIPLE_CALL_SLOT,
    CASE_HOLD_SLOT,
    CASE_ROTATE_SLOT,
];

/// A gesture record as reported by the buds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GestureRecord {
    pub device: u8,
    pub common: u8,
    pub kind: u8,
    pub action: u8,
}

pub fn device_code_for(target: Target, side: crate::types::Side) -> u8 {
    match target {
        Target::Case => DEVICE_CASE,
        Target::Buds => match side {
            crate::types::Side::Left => DEVICE_LEFT,
            crate::types::Side::Right => DEVICE_RIGHT,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anc_cycle_roundtrip() {
        for a in [10u8, 20, 21, 22] {
            assert_eq!(AncCycle::from_action(a).unwrap().to_action(), Some(a));
        }
        let one = AncCycle {
            transparency: true,
            noise_cancellation: false,
            off: false,
        };
        assert_eq!(one.to_action(), None);
        assert_eq!(AncCycle::default().to_action(), Some(22));
    }

    #[test]
    fn slot_matching() {
        assert!(BUD_SLOTS[0].matches(kind::DOUBLE, 1));
        assert!(BUD_SLOTS[0].matches(kind::DOUBLE, 5));
        assert!(CASE_DOUBLE.matches(kind::DOUBLE, 1));
        assert!(!CASE_DOUBLE.matches(kind::DOUBLE, 9));
        assert!(CASE_DOUBLE_CALL_SLOT.matches(kind::DOUBLE, 9));
        assert_eq!(BUD_SLOTS[2].label_for(21), Some("Noise control"));
    }
}
