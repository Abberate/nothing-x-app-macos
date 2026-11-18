//! Device state folded from [`Event`]s.

use std::collections::HashMap;

use crate::gesture::{AncCycle, GestureRecord, GestureSlot};
use crate::message::{AncKind, Event};
use crate::model::{Capabilities, ModelInfo};
use crate::types::*;

#[derive(Debug, Clone, Default)]
pub struct DeviceState {
    pub model: Option<&'static ModelInfo>,
    pub caps: Option<Capabilities>,
    pub firmware: Option<String>,
    pub serial: Option<String>,

    pub left: Option<BatteryLevel>,
    pub right: Option<BatteryLevel>,
    pub case: Option<BatteryLevel>,

    pub anc: Option<AncMode>,
    /// Last known strength, kept while ANC is off so turning it back on restores it.
    pub anc_strength: Option<AncStrength>,

    pub eq_preset: Option<EqPreset>,
    pub custom_eq: Option<CustomEq>,
    pub advanced_eq: bool,
    pub ultra_bass_enabled: Option<bool>,
    /// UI level (1..=5 or 0..=2 depending on the model's bass scale).
    pub ultra_bass_level: Option<u8>,

    pub in_ear_detection: Option<bool>,
    pub low_latency: Option<bool>,

    pub gestures: HashMap<(u8, u8, u8), u8>,

    pub ear_fit: Option<(EarFit, EarFit)>,
    pub ringing_left: bool,
    pub ringing_right: bool,

    pub dual_enabled: Option<bool>,
    pub dual_devices: Vec<DualDevice>,
    pub codec: Option<Codec>,
    pub spatial: Option<SpatialAudio>,
}

impl DeviceState {
    pub fn new(model: Option<&'static ModelInfo>) -> Self {
        Self {
            model,
            caps: model.and_then(|m| m.capabilities()),
            ..Default::default()
        }
    }

    /// Applies an event. Returns `true` when something visible changed.
    pub fn apply(&mut self, event: &Event) -> bool {
        match event {
            Event::Battery {
                left,
                right,
                case,
                single,
            } => {
                if single.is_some() {
                    self.left = *single;
                    self.right = None;
                } else {
                    self.left = *left;
                    self.right = *right;
                }
                self.case = *case;
            }
            Event::Anc(report) => {
                if let Some(s) = report.strength {
                    self.anc_strength = Some(s);
                }
                let strength = self.anc_strength.unwrap_or(AncStrength::High);
                match report.mode {
                    Some(AncKind::Off) => self.anc = Some(AncMode::Off),
                    Some(AncKind::Transparency) => self.anc = Some(AncMode::Transparency),
                    Some(AncKind::On) => self.anc = Some(AncMode::NoiseCancelling(strength)),
                    None => {
                        if let Some(AncMode::NoiseCancelling(_)) = self.anc {
                            self.anc = Some(AncMode::NoiseCancelling(strength));
                        }
                    }
                }
            }
            Event::InEarDetection(v) => self.in_ear_detection = Some(*v),
            Event::LowLatency(v) => self.low_latency = Some(*v),
            Event::Firmware(v) => self.firmware = Some(v.clone()),
            Event::Serial(v) => {
                if !v.is_empty() {
                    self.serial = Some(v.clone());
                }
            }
            Event::Eq(v) => self.eq_preset = EqPreset::from_wire(*v),
            Event::CustomEq(eq) => self.custom_eq = Some(*eq),
            Event::AdvancedEq(v) => self.advanced_eq = *v,
            Event::UltraBass {
                enabled,
                wire_level,
            } => {
                self.ultra_bass_enabled = Some(*enabled);
                if let Some(scale) = self.caps.and_then(|c| c.bass) {
                    self.ultra_bass_level = Some(scale.from_wire(*wire_level));
                }
            }
            Event::Gestures(records) => {
                for r in records {
                    self.gestures.insert((r.device, r.kind, r.common), r.action);
                }
            }
            Event::EarFit { left, right } => self.ear_fit = Some((*left, *right)),
            Event::Ringing(entries) => {
                for (pos, ringing) in entries {
                    match *pos {
                        DEVICE_LEFT | DEVICE_SINGLE => self.ringing_left = *ringing,
                        DEVICE_RIGHT => self.ringing_right = *ringing,
                        _ => {}
                    }
                }
            }
            Event::DualEnabled(v) => self.dual_enabled = Some(*v),
            Event::DualList { devices, .. } => {
                for d in devices {
                    match self.dual_devices.iter_mut().find(|x| x.mac == d.mac) {
                        Some(existing) => *existing = d.clone(),
                        None => self.dual_devices.push(d.clone()),
                    }
                }
            }
            Event::DualDevice { mac, connected } => {
                if let Some(d) = self.dual_devices.iter_mut().find(|d| d.mac == *mac) {
                    d.connected = *connected;
                }
            }
            Event::Codec(c) => self.codec = Some(*c),
            Event::SpatialAudio(s) => self.spatial = Some(*s),
            Event::Ack { .. } | Event::Unknown { .. } => return false,
        }
        true
    }

    /// Current action for a gesture slot on a device (left/right/case code).
    pub fn gesture_action(&self, device: u8, slot: &GestureSlot) -> Option<u8> {
        self.gestures
            .iter()
            .find(|((d, kind, common), _)| *d == device && slot.matches(*kind, *common))
            .map(|(_, a)| *a)
    }

    pub fn anc_cycle(&self, device: u8, slot: &GestureSlot) -> AncCycle {
        self.gesture_action(device, slot)
            .and_then(AncCycle::from_action)
            .unwrap_or_default()
    }

    /// Optimistically records a gesture we just wrote.
    pub fn record_gesture(&mut self, r: GestureRecord) {
        self.gestures
            .retain(|(d, k, c), _| !(*d == r.device && *k == r.kind && *c == r.common));
        self.gestures.insert((r.device, r.kind, r.common), r.action);
    }

    pub fn dirac_blocked(&self) -> bool {
        self.caps.is_some_and(|c| c.dirac_ldac_exclusive) && self.codec == Some(Codec::Ldac)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::AncReport;
    use crate::model::lookup;

    #[test]
    fn anc_strength_is_remembered() {
        let mut s = DeviceState::new(lookup(0x2F45F5));
        s.apply(&Event::Anc(AncReport {
            mode: Some(AncKind::On),
            strength: Some(AncStrength::Low),
        }));
        assert_eq!(s.anc, Some(AncMode::NoiseCancelling(AncStrength::Low)));
        s.apply(&Event::Anc(AncReport {
            mode: Some(AncKind::Off),
            strength: None,
        }));
        assert_eq!(s.anc, Some(AncMode::Off));
        s.apply(&Event::Anc(AncReport {
            mode: Some(AncKind::On),
            strength: None,
        }));
        assert_eq!(s.anc, Some(AncMode::NoiseCancelling(AncStrength::Low)));
    }

    #[test]
    fn ultra_bass_level_uses_model_scale() {
        let mut s = DeviceState::new(lookup(0x2F45F5));
        s.apply(&Event::UltraBass {
            enabled: true,
            wire_level: 8,
        });
        assert_eq!(s.ultra_bass_level, Some(4));
        let mut s = DeviceState::new(lookup(0xF51D3D));
        s.apply(&Event::UltraBass {
            enabled: true,
            wire_level: 10,
        });
        assert_eq!(s.ultra_bass_level, Some(2));
    }

    #[test]
    fn gestures_lookup() {
        let mut s = DeviceState::new(lookup(0x2F45F5));
        s.apply(&Event::Gestures(vec![GestureRecord {
            device: 2,
            common: 1,
            kind: 7,
            action: 21,
        }]));
        let slot = &crate::gesture::BUD_SLOTS[2];
        assert_eq!(s.gesture_action(2, slot), Some(21));
        let cycle = s.anc_cycle(2, slot);
        assert!(cycle.transparency && !cycle.noise_cancellation && cycle.off);
    }
}
