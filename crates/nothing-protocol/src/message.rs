//! Typed requests (what we send) and events (what the buds tell us).

use crate::cmd;
use crate::float;
use crate::frame::Frame;
use crate::gesture::GestureRecord;
use crate::model::{BassScale, CustomEqFilters};
use crate::types::*;

/// A request before framing (the session assigns the operation id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub command: u16,
    pub payload: Vec<u8>,
}

impl Request {
    pub fn get(command: u16) -> Self {
        Self {
            command,
            payload: Vec::new(),
        }
    }

    fn set(command: u16, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            command,
            payload: payload.into(),
        }
    }

    pub fn is_set(&self) -> bool {
        cmd::is_set(self.command)
    }

    pub fn encode(&self, operation: u8) -> Vec<u8> {
        Frame::encode(self.command, operation, &self.payload)
    }

    pub fn anc(mode: AncMode) -> Self {
        Self::set(cmd::SET_ANC, [0x01, mode.wire(), 0x00])
    }

    pub fn listening_mode(preset: EqPreset) -> Self {
        Self::set(cmd::SET_LISTENING_MODE, [preset as u8, 0x00])
    }

    /// Legacy EQ command (models without listening modes).
    pub fn eq(preset: u8) -> Self {
        Self::set(cmd::SET_EQ, [preset, 0x00])
    }

    /// 3-band custom EQ. Band order on the wire: peak (mid), high shelf (treble),
    /// low shelf (bass); a global gain of `-max(0, gains)` prevents clipping.
    pub fn custom_eq(eq: CustomEq, f: &CustomEqFilters) -> Self {
        const LOW_SHELF: u8 = 0;
        const PEAK: u8 = 1;
        const HIGH_SHELF: u8 = 2;
        let bands = [
            (PEAK, eq.mid, f.freq_peak, f.q_peak),
            (HIGH_SHELF, eq.treble, f.freq_high, f.q_high),
            (LOW_SHELF, eq.bass, f.freq_low, f.q_low),
        ];
        let max_gain = bands.iter().map(|b| b.1).fold(0.0f32, f32::max);
        let mut p = Vec::with_capacity(5 + bands.len() * 16);
        p.push(bands.len() as u8);
        p.extend_from_slice(&float::encode(-max_gain));
        for (kind, gain, freq, q) in bands {
            p.push(kind);
            p.extend_from_slice(&float::encode(gain));
            p.extend_from_slice(&float::encode(freq));
            p.extend_from_slice(&float::encode(q));
        }
        p.extend(std::iter::repeat_n(0u8, bands.len() * 3));
        Self::set(cmd::SET_CUSTOM_EQ, p)
    }

    pub fn ultra_bass(enabled: bool, level: u8, scale: BassScale) -> Self {
        Self::set(
            cmd::SET_ULTRA_BASS,
            [u8::from(enabled), scale.to_wire(level)],
        )
    }

    pub fn in_ear_detection(on: bool) -> Self {
        Self::set(cmd::SET_IN_EAR, [0x01, 0x01, u8::from(on)])
    }

    pub fn low_latency(on: bool) -> Self {
        Self::set(cmd::SET_LATENCY, [if on { 0x01 } else { 0x02 }, 0x00])
    }

    pub fn ring(side: Side, ring: bool) -> Self {
        Self::set(cmd::SET_RING, [side.code(), u8::from(ring)])
    }

    pub fn gesture(device: u8, common: u8, kind: u8, action: u8) -> Self {
        Self::set(cmd::SET_GESTURE, [0x01, device, common, kind, action])
    }

    pub fn ear_fit_test() -> Self {
        Self::set(cmd::SET_EAR_FIT_TEST, [0x01])
    }

    pub fn time(epoch_secs: u32) -> Self {
        Self::set(cmd::SET_TIME, epoch_secs.to_be_bytes())
    }

    pub fn dual_enable(on: bool) -> Self {
        Self::set(cmd::SET_DUAL_ENABLE, [u8::from(on)])
    }

    pub fn dual_list(offset: u8) -> Self {
        Self {
            command: cmd::GET_DUAL_LIST,
            payload: vec![offset],
        }
    }

    pub fn dual_connect(mac: [u8; 6], connect: bool) -> Self {
        let mut p = vec![u8::from(connect)];
        p.extend_from_slice(&mac);
        Self::set(cmd::SET_DUAL_CONNECT, p)
    }

    pub fn codec(codec: Codec) -> Self {
        Self::set(cmd::SET_CODEC, [codec as u8])
    }

    pub fn spatial_audio(mode: SpatialAudio) -> Self {
        let (m, h) = mode.wire();
        Self::set(cmd::SET_SPATIAL_AUDIO, [m, h])
    }
}

/// ANC report: list of `[type, value, _]` triples. Type 1 sets the mode (and strength
/// when on), type 2 only the strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AncReport {
    pub mode: Option<AncKind>,
    pub strength: Option<AncStrength>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AncKind {
    Off,
    Transparency,
    On,
}

/// Something the buds told us.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Battery {
        left: Option<BatteryLevel>,
        right: Option<BatteryLevel>,
        case: Option<BatteryLevel>,
        single: Option<BatteryLevel>,
    },
    Anc(AncReport),
    InEarDetection(bool),
    LowLatency(bool),
    Firmware(String),
    Serial(String),
    /// Raw preset byte (`EqPreset::from_wire` for listening-mode models).
    Eq(u8),
    CustomEq(CustomEq),
    AdvancedEq(bool),
    UltraBass {
        enabled: bool,
        wire_level: u8,
    },
    Gestures(Vec<GestureRecord>),
    EarFit {
        left: EarFit,
        right: EarFit,
    },
    Ringing(Vec<(u8, bool)>),
    DualEnabled(bool),
    DualList {
        total: Option<u8>,
        devices: Vec<DualDevice>,
    },
    DualDevice {
        mac: [u8; 6],
        connected: bool,
    },
    Codec(Codec),
    SpatialAudio(SpatialAudio),
    /// Write acknowledgement (`0x7xxx`).
    Ack {
        command: u16,
        operation: u8,
    },
    Unknown {
        command: u16,
        payload: Vec<u8>,
    },
}

impl Event {
    /// Decodes a frame; malformed payloads become [`Event::Unknown`] rather than panicking.
    pub fn parse(frame: &Frame) -> Event {
        let p = frame.payload.as_slice();
        let unknown = || Event::Unknown {
            command: frame.command,
            payload: frame.payload.clone(),
        };
        if cmd::is_write_ack(frame.command) {
            return Event::Ack {
                command: frame.command,
                operation: frame.operation,
            };
        }
        let parsed = match cmd::key(frame.command) {
            cmd::GET_BATTERY | cmd::PUSH_BATTERY => parse_battery(p),
            cmd::GET_ANC | cmd::PUSH_ANC => Some(Event::Anc(parse_anc(p))),
            cmd::GET_IN_EAR => p.get(2).map(|v| Event::InEarDetection(*v != 0)),
            cmd::GET_LATENCY => p.first().map(|v| Event::LowLatency(*v == 1)),
            cmd::GET_FIRMWARE => Some(Event::Firmware(ascii(p))),
            cmd::GET_SERIAL => Some(Event::Serial(parse_serial(p))),
            cmd::GET_EQ | cmd::GET_LISTENING_MODE => p.first().map(|v| Event::Eq(*v)),
            cmd::GET_CUSTOM_EQ => parse_custom_eq(p),
            cmd::GET_ADVANCED_EQ_STATUS => p.first().map(|v| Event::AdvancedEq(*v == 1)),
            cmd::GET_ULTRA_BASS => (p.len() >= 2).then(|| Event::UltraBass {
                enabled: p[0] == 1,
                wire_level: p[1],
            }),
            cmd::GET_GESTURES => Some(Event::Gestures(parse_gestures(p))),
            cmd::PUSH_EAR_FIT => (p.len() >= 2).then(|| Event::EarFit {
                left: EarFit::from_wire(p[0]),
                right: EarFit::from_wire(p[1]),
            }),
            cmd::GET_FIND_STATE => parse_find_state(p),
            cmd::GET_DUAL_ENABLE => p.first().map(|v| Event::DualEnabled(*v == 1)),
            cmd::GET_DUAL_LIST => parse_dual_list(p),
            cmd::PUSH_DUAL_DEVICE => (p.len() >= 7).then(|| {
                let mut mac = [0u8; 6];
                mac.copy_from_slice(&p[1..7]);
                Event::DualDevice {
                    mac,
                    connected: p[0] == 1,
                }
            }),
            cmd::GET_CODEC => Some(Event::Codec(Codec::from_wire(
                p.first().copied().unwrap_or(0),
            ))),
            cmd::GET_SPATIAL_AUDIO => Some(Event::SpatialAudio(SpatialAudio::from_wire(
                p.first().copied().unwrap_or(0),
                p.get(1).copied().unwrap_or(0),
            ))),
            _ => None,
        };
        parsed.unwrap_or_else(unknown)
    }
}

fn ascii(p: &[u8]) -> String {
    p.iter()
        .take_while(|b| **b != 0)
        .map(|b| *b as char)
        .collect::<String>()
        .trim()
        .to_string()
}

fn parse_battery(p: &[u8]) -> Option<Event> {
    let count = *p.first()? as usize;
    let (mut left, mut right, mut case, mut single) = (None, None, None, None);
    for i in 0..count {
        let (Some(&id), Some(&raw)) = (p.get(1 + i * 2), p.get(2 + i * 2)) else {
            break;
        };
        let level = BatteryLevel {
            percent: (raw & 0x7F).min(100),
            charging: raw & 0x80 != 0,
        };
        match id {
            DEVICE_LEFT => left = Some(level),
            DEVICE_RIGHT => right = Some(level),
            DEVICE_CASE => case = Some(level),
            DEVICE_SINGLE => single = Some(level),
            _ => {}
        }
    }
    Some(Event::Battery {
        left,
        right,
        case,
        single,
    })
}

fn parse_anc(p: &[u8]) -> AncReport {
    let mut report = AncReport::default();
    let mut i = 0;
    while i + 1 < p.len() {
        let (kind, value) = (p[i], p[i + 1]);
        match kind {
            0x01 => match value {
                1..=4 => {
                    report.mode = Some(AncKind::On);
                    report.strength = AncStrength::from_wire(value);
                }
                0 | 5 => report.mode = Some(AncKind::Off),
                7 | 0xFE => report.mode = Some(AncKind::Transparency),
                _ => {}
            },
            0x02 => {
                if let Some(s) = AncStrength::from_wire(value) {
                    report.strength = Some(s);
                }
            }
            _ => {}
        }
        i += 3;
    }
    report
}

/// Layout mirrors the write packet: `[count][total gain f32]` then 13-byte bands
/// `[type][gain][freq][q]` in order peak (mid), high shelf (treble), low shelf (bass).
fn parse_custom_eq(p: &[u8]) -> Option<Event> {
    let gain = |i: usize| float::read(p, 6 + i * 13);
    let (mid, treble, bass) = (gain(0)?, gain(1)?, gain(2)?);
    let clean = |v: f32| {
        if v.is_finite() {
            v.clamp(CustomEq::MIN, CustomEq::MAX)
        } else {
            0.0
        }
    };
    Some(Event::CustomEq(CustomEq {
        bass: clean(bass),
        mid: clean(mid),
        treble: clean(treble),
    }))
}

fn parse_gestures(p: &[u8]) -> Vec<GestureRecord> {
    let count = p.first().copied().unwrap_or(0) as usize;
    (0..count)
        .filter_map(|i| {
            let r = p.get(1 + i * 4..5 + i * 4)?;
            Some(GestureRecord {
                device: r[0],
                common: r[1],
                kind: r[2],
                action: r[3],
            })
        })
        .collect()
}

fn parse_find_state(p: &[u8]) -> Option<Event> {
    let count = *p.first()? as usize;
    if p.len() != 1 + 2 * count {
        return None;
    }
    Some(Event::Ringing(
        p[1..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| (c[0], c[1] == 1))
            .collect(),
    ))
}

/// `[_, _, count]` then per device `[flags][mac 6][name_len][name…]`.
fn parse_dual_list(p: &[u8]) -> Option<Event> {
    if p.len() < 3 {
        return None;
    }
    let count = p[2] as usize;
    let mut devices = Vec::new();
    let mut off = 3;
    for _ in 0..count {
        if off + 8 > p.len() {
            break;
        }
        let flags = p[off];
        let mut mac = [0u8; 6];
        mac.copy_from_slice(&p[off + 1..off + 7]);
        let name_len = (p[off + 7] & 0x7F) as usize;
        let end = (off + 8 + name_len).min(p.len());
        let name = String::from_utf8_lossy(&p[off + 8..end]).into_owned();
        devices.push(DualDevice {
            mac,
            name,
            is_self: flags & 0xF0 != 0,
            connected: flags & 0x0F != 0,
        });
        off = end;
    }
    Some(Event::DualList {
        total: p.get(1).copied(),
        devices,
    })
}

/// Serial number response: text lines `device,type,value`; type 4 is the serial.
fn parse_serial(p: &[u8]) -> String {
    let text = String::from_utf8_lossy(p);
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, ',');
            let (_dev, kind, value) = (parts.next()?, parts.next()?, parts.next()?);
            (kind.trim() == "4" && !value.trim().is_empty()).then(|| value.trim().to_string())
        })
        .next()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::lookup;

    fn frame(command: u16, payload: &[u8]) -> Frame {
        Frame {
            command,
            operation: 1,
            payload: payload.to_vec(),
        }
    }

    #[test]
    fn battery() {
        let e = Event::parse(&frame(0x4007, &[3, 2, 95, 3, 0x80 | 60, 4, 40]));
        assert_eq!(
            e,
            Event::Battery {
                left: Some(BatteryLevel {
                    percent: 95,
                    charging: false
                }),
                right: Some(BatteryLevel {
                    percent: 60,
                    charging: true
                }),
                case: Some(BatteryLevel {
                    percent: 40,
                    charging: false
                }),
                single: None,
            }
        );
        // Push variant and truncated payload must not panic.
        assert!(matches!(
            Event::parse(&frame(0xE001, &[3, 2])),
            Event::Battery { .. }
        ));
    }

    #[test]
    fn anc_variants() {
        let on_mid = Event::parse(&frame(0x401E, &[1, 2, 0]));
        assert_eq!(
            on_mid,
            Event::Anc(AncReport {
                mode: Some(AncKind::On),
                strength: Some(AncStrength::Mid)
            })
        );
        let transp = Event::parse(&frame(0xE003, &[1, 7, 0]));
        assert!(matches!(
            transp,
            Event::Anc(AncReport {
                mode: Some(AncKind::Transparency),
                ..
            })
        ));
        let strength_only = Event::parse(&frame(0x401E, &[2, 4, 0]));
        assert_eq!(
            strength_only,
            Event::Anc(AncReport {
                mode: None,
                strength: Some(AncStrength::Adaptive)
            })
        );
    }

    #[test]
    fn request_payloads() {
        assert_eq!(
            Request::anc(AncMode::NoiseCancelling(AncStrength::Adaptive)).payload,
            vec![1, 4, 0]
        );
        assert_eq!(Request::anc(AncMode::Off).payload, vec![1, 5, 0]);
        assert_eq!(Request::anc(AncMode::Transparency).payload, vec![1, 7, 0]);
        assert_eq!(Request::listening_mode(EqPreset::Pop).payload, vec![3, 0]);
        assert_eq!(Request::low_latency(false).payload, vec![2, 0]);
        assert_eq!(Request::in_ear_detection(true).payload, vec![1, 1, 1]);
        assert_eq!(Request::ring(Side::Right, true).payload, vec![3, 1]);
        assert_eq!(
            Request::ultra_bass(true, 3, BassScale::FiveLevel).payload,
            vec![1, 6]
        );
        assert_eq!(Request::time(0x01020304).payload, vec![1, 2, 3, 4]);
        assert_eq!(Request::gesture(2, 1, 7, 22).payload, vec![1, 2, 1, 7, 22]);
    }

    #[test]
    fn custom_eq_roundtrip() {
        let filters = lookup(0x2F45F5)
            .unwrap()
            .capabilities()
            .unwrap()
            .custom_eq
            .unwrap();
        let eq = CustomEq {
            bass: 3.0,
            mid: -2.0,
            treble: 5.0,
        };
        let req = Request::custom_eq(eq, &filters);
        assert_eq!(req.command, cmd::SET_CUSTOM_EQ);
        assert_eq!(req.payload.len(), 5 + 3 * 16);
        assert_eq!(req.payload[0], 3);
        assert_eq!(float::read(&req.payload, 1), Some(-5.0));
        assert_eq!(req.payload[5], 1); // peak first
        // The read response uses the same layout as the write.
        let parsed = Event::parse(&frame(0x4044, &req.payload));
        assert_eq!(parsed, Event::CustomEq(eq));
    }

    #[test]
    fn gestures_and_misc() {
        let g = Event::parse(&frame(0x4018, &[2, 2, 1, 2, 9, 3, 1, 7, 22]));
        assert_eq!(
            g,
            Event::Gestures(vec![
                GestureRecord {
                    device: 2,
                    common: 1,
                    kind: 2,
                    action: 9
                },
                GestureRecord {
                    device: 3,
                    common: 1,
                    kind: 7,
                    action: 22
                },
            ])
        );
        assert_eq!(
            Event::parse(&frame(0x4042, b"1.0.0.52")),
            Event::Firmware("1.0.0.52".into())
        );
        assert_eq!(
            Event::parse(&frame(0x400E, &[1, 1, 1])),
            Event::InEarDetection(true)
        );
        assert_eq!(Event::parse(&frame(0x4041, &[2])), Event::LowLatency(false));
        assert_eq!(
            Event::parse(&frame(0x404E, &[1, 8])),
            Event::UltraBass {
                enabled: true,
                wire_level: 8
            }
        );
        assert_eq!(
            Event::parse(&frame(0xE00D, &[0, 1])),
            Event::EarFit {
                left: EarFit::Good,
                right: EarFit::Adjust
            }
        );
        assert_eq!(
            Event::parse(&frame(0x4002, &[2, 2, 1, 3, 0])),
            Event::Ringing(vec![(2, true), (3, false)])
        );
        assert_eq!(
            Event::parse(&frame(0x700F, &[])),
            Event::Ack {
                command: 0x700F,
                operation: 1
            }
        );
        assert_eq!(Event::parse(&frame(0x4050, &[6])), Event::Eq(6));
    }

    /// Payloads captured from a CMF Buds Pro 2 (firmware 1.0.1.74).
    #[test]
    fn real_cmf_buds_pro_2_payloads() {
        use crate::state::DeviceState;
        let h = |s: &str| -> Vec<u8> {
            s.split_whitespace()
                .map(|b| u8::from_str_radix(b, 16).unwrap())
                .collect()
        };
        let mut st = DeviceState::new(lookup(0x2F45F5));
        let frames = [
            (0x4007, "02 02 64 03 64"),
            (0x4050, "00"),
            (0x401E, "02 01 00 01 07 00"),
            (
                0x400E,
                "0e 01 01 02 00 03 01 07 01 09 01 0a 00 0b 00 0c 01 0e 01 0f 01 12 01 15 01 16 01 18 01",
            ),
            (0x4041, "02"),
            (0x404E, "01 06"),
            (0x4029, "02"),
            (0x404F, "01"),
            (0x4027, "01"),
            (0x4044, "00 00 00 00 00"),
            (
                0x4018,
                "0f 02 01 02 09 02 01 03 08 02 01 07 0a 02 01 09 13 03 01 02 09 03 01 03 08 03 01 07 0a 03 01 09 12 04 01 01 02 04 01 02 09 04 09 02 03 04 01 03 08 04 09 03 01 04 01 07 0a 04 01 0a 17",
            ),
            (
                0x4028,
                "01 00 02 11 11 22 33 44 55 66 0b 4d 61 63 42 6f 6f 6b 20 50 72 6f 01 aa bb cc dd ee ff 0a 47 61 6c 61 78 79 20 53 32 33",
            ),
        ];
        for (command, payload) in frames {
            st.apply(&Event::parse(&frame(command, &h(payload))));
        }
        st.apply(&Event::parse(&frame(0x4042, b"1.0.1.74")));
        assert_eq!(
            st.left,
            Some(BatteryLevel {
                percent: 100,
                charging: false
            })
        );
        assert_eq!(
            st.right,
            Some(BatteryLevel {
                percent: 100,
                charging: false
            })
        );
        assert_eq!(st.eq_preset, Some(EqPreset::DiracOpteo));
        assert_eq!(st.anc, Some(AncMode::Transparency));
        assert_eq!(st.anc_strength, Some(AncStrength::High));
        assert_eq!(st.in_ear_detection, Some(true));
        assert_eq!(st.low_latency, Some(false));
        assert_eq!(
            (st.ultra_bass_enabled, st.ultra_bass_level),
            (Some(true), Some(3))
        );
        assert_eq!(st.codec, Some(Codec::Ldac));
        assert_eq!(st.spatial, Some(SpatialAudio::Fixed));
        assert_eq!(st.dual_enabled, Some(true));
        assert_eq!(st.custom_eq, None); // never configured: empty response
        assert_eq!(st.firmware.as_deref(), Some("1.0.1.74"));
        assert_eq!(st.gestures.len(), 15);
        assert_eq!(st.gestures.get(&(2, 7, 1)), Some(&10));
        assert_eq!(st.gestures.get(&(4, 10, 1)), Some(&23));
        assert_eq!(st.dual_devices.len(), 2);
        assert!(st.dual_devices[0].is_self && st.dual_devices[0].connected);
        assert_eq!(st.dual_devices[1].name, "Galaxy S23");
        // B187 revision: Dirac stays available with LDAC.
        assert!(!st.dirac_blocked());
    }

    #[test]
    fn dual_list() {
        let mut p = vec![0, 2, 2];
        p.extend_from_slice(&[0x11, 1, 2, 3, 4, 5, 6, 3]);
        p.extend_from_slice(b"Mac");
        p.extend_from_slice(&[0x00, 9, 9, 9, 9, 9, 9, 5]);
        p.extend_from_slice(b"Phone");
        let Event::DualList { devices, .. } = Event::parse(&frame(0x4028, &p)) else {
            panic!("expected dual list")
        };
        assert_eq!(devices.len(), 2);
        assert!(devices[0].is_self && devices[0].connected);
        assert_eq!(devices[0].mac_string(), "01-02-03-04-05-06");
        assert_eq!(devices[1].name, "Phone");
        assert!(!devices[1].connected);
    }
}
