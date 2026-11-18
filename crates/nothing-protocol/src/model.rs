//! Model identification (Fast Pair model id) and per-model capabilities.

/// A product variant, identified by its Google Fast Pair model id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelInfo {
    pub id: u32,
    pub name: &'static str,
    /// Protocol family ("modelBase"): gates which commands the firmware understands.
    pub base: &'static str,
    /// Hardware revision whose configuration overrides the base one, if any.
    pub alt_base: Option<&'static str>,
    /// Asset key, e.g. `b187_black` → `b187_black_left.webp`.
    pub image: &'static str,
}

impl ModelInfo {
    pub fn config_id(&self) -> &'static str {
        self.alt_base.unwrap_or(self.base)
    }

    pub fn id_hex(&self) -> String {
        format!("{:06X}", self.id)
    }

    /// Short name shown in dot-matrix at the top ("CMF Buds Pro 2" → "BUDS PRO 2").
    pub fn display_title(&self) -> String {
        let name = self.name.trim();
        let name = name
            .strip_prefix("CMF ")
            .or_else(|| name.strip_prefix("Nothing "))
            .unwrap_or(name);
        name.to_uppercase()
    }

    pub fn capabilities(&self) -> Option<Capabilities> {
        Capabilities::for_model(self)
    }

    pub fn is_supported(&self) -> bool {
        self.capabilities().is_some()
    }
}

pub fn lookup(id: u32) -> Option<&'static ModelInfo> {
    FASTPAIR_TABLE.iter().find(|m| m.id == id)
}

/// Fallback when the Fast Pair channel is unavailable: match the Bluetooth name.
pub fn guess_from_name(name: &str) -> Option<&'static ModelInfo> {
    let n = name.trim().to_lowercase();
    // Most specific names first ("CMF Buds Pro 2" before "CMF Buds Pro").
    let mut candidates: Vec<&'static ModelInfo> = FASTPAIR_TABLE
        .iter()
        .filter(|m| n.contains(&m.name.to_lowercase()))
        .collect();
    candidates.sort_by_key(|m| std::cmp::Reverse(m.name.len()));
    candidates.first().copied()
}

/// Ultra Bass level encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BassScale {
    /// UI levels 1..=5, wire byte = level × 2.
    FiveLevel,
    /// UI levels 0..=2, wire bytes 0 / 5 / 10 (level 0 = off).
    ThreeStage,
}

impl BassScale {
    pub fn levels(self) -> std::ops::RangeInclusive<u8> {
        match self {
            Self::FiveLevel => 1..=5,
            Self::ThreeStage => 0..=2,
        }
    }

    pub fn to_wire(self, level: u8) -> u8 {
        match self {
            Self::FiveLevel => level.saturating_mul(2),
            Self::ThreeStage => [0u8, 5, 10].get(level as usize).copied().unwrap_or(0),
        }
    }

    pub fn from_wire(self, wire: u8) -> u8 {
        match self {
            Self::FiveLevel => (wire / 2).clamp(1, 5),
            Self::ThreeStage => match wire {
                0..=2 => 0,
                3..=7 => 1,
                _ => 2,
            },
        }
    }
}

/// Parametric filters behind the 3-band custom EQ (from the model configuration).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CustomEqFilters {
    pub freq_low: f32,
    pub q_low: f32,
    pub freq_peak: f32,
    pub q_peak: f32,
    pub freq_high: f32,
    pub q_high: f32,
}

/// What a model supports, mirrored from the reference configuration file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Capabilities {
    pub base: &'static str,
    pub config_id: &'static str,
    /// Bitmask: 0x10 transparency, 0x04 high/low, 0x02 mid, 0x08 adaptive.
    pub anc_level: u8,
    pub bass: Option<BassScale>,
    /// Uses `GET/SET_LISTENING_MODE` (0xC050/0xF01D) for presets.
    pub listening_mode: bool,
    pub custom_eq: Option<CustomEqFilters>,
    /// Dirac OPTEO cannot be used together with LDAC (`diracOpteoSupport == 1`).
    pub dirac_ldac_exclusive: bool,
    pub ear_tip_test: bool,
    pub in_ear_detection: bool,
    pub smart_dial: bool,
    pub dual_connection: bool,
    pub dual_connection_reboot: bool,
    /// Bitmask: 0x02 LHDC, 0x04 LDAC (AAC always available).
    pub codecs: u8,
    /// Bitmask of [`crate::SpatialAudio::capability_bit`].
    pub spatial_audio: u8,
    pub has_case: bool,
}

impl Capabilities {
    pub fn anc_transparency(&self) -> bool {
        self.anc_level & 0x10 != 0
    }
    pub fn anc_strength(&self) -> bool {
        self.anc_level & 0x04 != 0
    }
    pub fn anc_mid(&self) -> bool {
        self.anc_level & 0x02 != 0
    }
    pub fn anc_adaptive(&self) -> bool {
        self.anc_level & 0x08 != 0
    }

    pub fn available_strengths(&self) -> Vec<crate::AncStrength> {
        use crate::AncStrength::*;
        let mut v = Vec::new();
        if self.anc_strength() {
            v.push(High);
            if self.anc_mid() {
                v.push(Mid);
            }
            v.push(Low);
            if self.anc_adaptive() {
                v.push(Adaptive);
            }
        }
        v
    }

    pub fn available_codecs(&self) -> Vec<crate::Codec> {
        let mut v = vec![crate::Codec::Aac];
        if self.codecs & 0x02 != 0 {
            v.push(crate::Codec::Lhdc);
        }
        if self.codecs & 0x04 != 0 {
            v.push(crate::Codec::Ldac);
        }
        v
    }

    pub fn available_spatial(&self) -> Vec<crate::SpatialAudio> {
        crate::SpatialAudio::ALL
            .into_iter()
            .filter(|m| {
                m.capability_bit()
                    .is_none_or(|b| self.spatial_audio & b != 0)
            })
            .collect()
    }

    pub fn for_model(model: &ModelInfo) -> Option<Self> {
        let filters = CustomEqFilters {
            freq_low: 140.0,
            q_low: 0.8,
            freq_peak: 980.0,
            q_peak: 0.7,
            freq_high: 6900.0,
            q_high: 1.0,
        };
        // CMF Buds Pro 2 family ("espeon"): B172, its B187 revision, and B195.
        let espeon = Capabilities {
            base: model.base,
            config_id: model.config_id(),
            anc_level: 63,
            bass: Some(BassScale::FiveLevel),
            listening_mode: true,
            custom_eq: Some(filters),
            dirac_ldac_exclusive: true,
            ear_tip_test: true,
            in_ear_detection: true,
            smart_dial: true,
            dual_connection: true,
            dual_connection_reboot: true,
            codecs: 4,
            spatial_audio: 6,
            has_case: true,
        };
        match (model.base, model.config_id()) {
            ("B172", "B172") => Some(espeon),
            ("B172", "B187") => Some(Capabilities {
                dirac_ldac_exclusive: false,
                ..espeon
            }),
            ("B195", _) => Some(Capabilities {
                dirac_ldac_exclusive: false,
                dual_connection_reboot: false,
                bass: Some(BassScale::ThreeStage),
                ..espeon
            }),
            _ => None,
        }
    }
}

#[rustfmt::skip]
pub(crate) const FASTPAIR_TABLE: &[ModelInfo] = &[
    ModelInfo { id: 0x31D53D, name: "Nothing ear (1)", base: "B181", alt_base: None, image: "b181_white" },
    ModelInfo { id: 0x624011, name: "Nothing ear (1)", base: "B181", alt_base: None, image: "b181_black" },
    ModelInfo { id: 0x1016DD, name: "Ear (Stick)", base: "B157", alt_base: None, image: "b157_white" },
    ModelInfo { id: 0xACC520, name: "Ear (2)", base: "B155", alt_base: None, image: "b155_white" },
    ModelInfo { id: 0xDEE8C0, name: "Ear (2)", base: "B155", alt_base: None, image: "b155_black" },
    ModelInfo { id: 0xFEB1C7, name: "Nothing Ear", base: "B171", alt_base: None, image: "b171_white" },
    ModelInfo { id: 0xA20444, name: "Nothing Ear", base: "B171", alt_base: None, image: "b171_black" },
    ModelInfo { id: 0x5E3FBC, name: "Nothing Ear (a)", base: "B162", alt_base: None, image: "b162_white" },
    ModelInfo { id: 0x03464E, name: "Nothing Ear (a)", base: "B162", alt_base: None, image: "b162_black" },
    ModelInfo { id: 0x8B6380, name: "Nothing Ear (a)", base: "B162", alt_base: None, image: "b162_yellow" },
    ModelInfo { id: 0xFC3AAF, name: "Nothing Ear (open)", base: "B174", alt_base: None, image: "b174_white" },
    ModelInfo { id: 0x5F8F82, name: "Buds Pro", base: "B163", alt_base: None, image: "b163_orange" },
    ModelInfo { id: 0x2EB1CA, name: "Buds Pro", base: "B163", alt_base: None, image: "b163_white" },
    ModelInfo { id: 0xADD2C4, name: "Buds Pro", base: "B163", alt_base: None, image: "b163_black" },
    ModelInfo { id: 0x4DFC4A, name: "Neckband Pro", base: "B164", alt_base: None, image: "b164_orange" },
    ModelInfo { id: 0x26C190, name: "Neckband Pro", base: "B164", alt_base: None, image: "b164_white" },
    ModelInfo { id: 0xAE35FD, name: "Neckband Pro", base: "B164", alt_base: None, image: "b164_black" },
    ModelInfo { id: 0xD35E18, name: "CMF Buds", base: "B168", alt_base: None, image: "b168_orange" },
    ModelInfo { id: 0xACCE54, name: "CMF Buds", base: "B168", alt_base: None, image: "b168_white" },
    ModelInfo { id: 0x150A27, name: "CMF Buds", base: "B168", alt_base: None, image: "b168_black" },
    ModelInfo { id: 0xF29566, name: "CMF Buds Pro 2", base: "B172", alt_base: None, image: "b172_black" },
    ModelInfo { id: 0xCA36A6, name: "CMF Buds Pro 2", base: "B172", alt_base: None, image: "b172_white" },
    ModelInfo { id: 0xA7B220, name: "CMF Buds Pro 2", base: "B172", alt_base: None, image: "b172_orange" },
    ModelInfo { id: 0x2B353E, name: "CMF Buds Pro 2", base: "B172", alt_base: None, image: "b172_blue" },
    ModelInfo { id: 0xC34F3B, name: "Nothing Ear (a)", base: "B162", alt_base: Some("B183"), image: "b183_black" },
    ModelInfo { id: 0x404D6D, name: "Nothing Ear (a)", base: "B162", alt_base: Some("B183"), image: "b183_white" },
    ModelInfo { id: 0x839E9A, name: "Nothing Ear (a)", base: "B162", alt_base: Some("B183"), image: "b183_yellow" },
    ModelInfo { id: 0x19EF24, name: "CMF Buds 2", base: "B179", alt_base: None, image: "b179_black" },
    ModelInfo { id: 0xFF2AB0, name: "CMF Buds 2", base: "B179", alt_base: None, image: "b179_green" },
    ModelInfo { id: 0xD9AB5D, name: "CMF Buds 2", base: "B179", alt_base: None, image: "b179_orange" },
    ModelInfo { id: 0x3B1320, name: "CMF Buds 2", base: "B197", alt_base: None, image: "b179_black" },
    ModelInfo { id: 0x359A7D, name: "CMF Buds 2", base: "B197", alt_base: None, image: "b179_green" },
    ModelInfo { id: 0x00FDED, name: "CMF Buds 2", base: "B197", alt_base: None, image: "b179_orange" },
    ModelInfo { id: 0x5C587F, name: "CMF Buds 2 Plus", base: "B184", alt_base: None, image: "b184_blue" },
    ModelInfo { id: 0x4AEB6E, name: "CMF Buds 2 Plus", base: "B184", alt_base: None, image: "b184_white" },
    ModelInfo { id: 0x70F8E3, name: "CMF Buds 2a", base: "B185", alt_base: None, image: "b185_black" },
    ModelInfo { id: 0xED5412, name: "CMF Buds 2a", base: "B185", alt_base: None, image: "b185_white" },
    ModelInfo { id: 0x509CAE, name: "CMF Buds 2a", base: "B185", alt_base: None, image: "b185_orange" },
    ModelInfo { id: 0x2F45F5, name: "CMF Buds Pro 2", base: "B172", alt_base: Some("B187"), image: "b187_black" },
    ModelInfo { id: 0xE1BE45, name: "CMF Buds Pro 2", base: "B172", alt_base: Some("B187"), image: "b187_white" },
    ModelInfo { id: 0x1253C0, name: "CMF Buds Pro 2", base: "B172", alt_base: Some("B187"), image: "b187_orange" },
    ModelInfo { id: 0x0F1A4F, name: "CMF Buds Pro 2", base: "B172", alt_base: Some("B187"), image: "b187_blue" },
    ModelInfo { id: 0xF51D3D, name: "CMF Buds Pro 2", base: "B195", alt_base: None, image: "b187_black" },
    ModelInfo { id: 0xA544CE, name: "CMF Buds Pro 2", base: "B195", alt_base: None, image: "b187_white" },
    ModelInfo { id: 0x9EAC04, name: "CMF Buds Pro 2", base: "B195", alt_base: None, image: "b187_orange" },
    ModelInfo { id: 0x1D07D4, name: "CMF Buds Pro 2", base: "B195", alt_base: None, image: "b187_blue" },
    ModelInfo { id: 0x53BFE6, name: "CMF Buds Pro 2", base: "B195", alt_base: None, image: "b187_green" },
    ModelInfo { id: 0x072714, name: "CMF Buds Pro 2", base: "B195", alt_base: None, image: "b187_pink" },
    ModelInfo { id: 0xC19ECD, name: "Nothing Headphone (1)", base: "B170", alt_base: None, image: "b170_black" },
    ModelInfo { id: 0x2D6FDA, name: "Nothing Headphone (1)", base: "B170", alt_base: None, image: "b170_grey" },
    ModelInfo { id: 0x7D46E5, name: "Nothing Ear (3)", base: "B173", alt_base: None, image: "b173_black" },
    ModelInfo { id: 0xC1EBFD, name: "Nothing Ear (3)", base: "B173", alt_base: None, image: "b173_white" },
    ModelInfo { id: 0x056133, name: "Nothing Ear (3)", base: "B173", alt_base: Some("B201"), image: "b173_black" },
    ModelInfo { id: 0x77FDD8, name: "Nothing Ear (3)", base: "B173", alt_base: Some("B201"), image: "b173_white" },
    ModelInfo { id: 0x563DA5, name: "CMF Headphone Pro", base: "B175", alt_base: None, image: "b175_green" },
    ModelInfo { id: 0x73C9EB, name: "CMF Headphone Pro", base: "B175", alt_base: None, image: "b175_white" },
    ModelInfo { id: 0x1EFB39, name: "CMF Headphone Pro", base: "B175", alt_base: None, image: "b175_black" },
    ModelInfo { id: 0xBFD53B, name: "Nothing Headphone (a)", base: "B186", alt_base: None, image: "b186_black" },
    ModelInfo { id: 0xDE8953, name: "Nothing Headphone (a)", base: "B186", alt_base: None, image: "b186_white" },
    ModelInfo { id: 0x97EF75, name: "Nothing Headphone (a)", base: "B186", alt_base: Some("B198"), image: "b198_yellow" },
    ModelInfo { id: 0x98D02B, name: "Nothing Headphone (a)", base: "B186", alt_base: None, image: "b186_pink" },
    ModelInfo { id: 0x6F6C71, name: "Nothing Headphone (a)", base: "B186", alt_base: Some("B198"), image: "b198_black" },
    ModelInfo { id: 0x810478, name: "Nothing Headphone (a)", base: "B186", alt_base: Some("B198"), image: "b198_white" },
    ModelInfo { id: 0x79B3A9, name: "Nothing Headphone (a)", base: "B186", alt_base: Some("B198"), image: "b198_yellow" },
    ModelInfo { id: 0xA292C6, name: "Nothing Headphone (a)", base: "B186", alt_base: Some("B198"), image: "b198_pink" },
    ModelInfo { id: 0xCC3444, name: "Nothing Ear (open)", base: "B174", alt_base: None, image: "b174_blue" },
    ModelInfo { id: 0x7B2328, name: "Nothing Ear (3a)", base: "B190", alt_base: None, image: "b190_white" },
    ModelInfo { id: 0xE9C3BE, name: "Nothing Ear (3a)", base: "B190", alt_base: None, image: "b190_black" },
    ModelInfo { id: 0x148887, name: "Nothing Ear (3a)", base: "B190", alt_base: None, image: "b190_pink" },
    ModelInfo { id: 0xDB45D3, name: "Nothing Ear (3a)", base: "B190", alt_base: None, image: "b190_yellow" },
    ModelInfo { id: 0x8F28ED, name: "CMF Clip Pro", base: "B189", alt_base: None, image: "b189_blue" },
    ModelInfo { id: 0xE6673A, name: "CMF Clip Pro", base: "B189", alt_base: None, image: "b189_orange" },
    ModelInfo { id: 0xDCD2CB, name: "CMF Clip Pro", base: "B189", alt_base: None, image: "b189_white" },
    ModelInfo { id: 0xDA1280, name: "CMF Clip Pro", base: "B189", alt_base: None, image: "b189_black" },
    ModelInfo { id: 0xCDFDEC, name: "Nothing Headphone (1) Pro", base: "B192", alt_base: None, image: "b192_black" },
    ModelInfo { id: 0x086485, name: "Nothing Headphone (1) Pro", base: "B192", alt_base: None, image: "b192_silver" },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn your_buds() {
        let m = lookup(0x2F45F5).expect("CMF Buds Pro 2 (B187 black)");
        assert_eq!(m.name, "CMF Buds Pro 2");
        assert_eq!(m.base, "B172");
        assert_eq!(m.config_id(), "B187");
        assert_eq!(m.display_title(), "BUDS PRO 2");
        let caps = m.capabilities().unwrap();
        assert!(!caps.dirac_ldac_exclusive);
        assert_eq!(caps.available_strengths().len(), 4);
        assert_eq!(
            caps.available_codecs(),
            vec![crate::Codec::Aac, crate::Codec::Ldac]
        );
        assert_eq!(
            caps.available_spatial(),
            vec![crate::SpatialAudio::Off, crate::SpatialAudio::Fixed]
        );
    }

    #[test]
    fn name_guess_prefers_longest() {
        assert_eq!(
            guess_from_name("CMF Buds Pro 2").unwrap().name,
            "CMF Buds Pro 2"
        );
    }

    #[test]
    fn bass_scales() {
        assert_eq!(BassScale::FiveLevel.to_wire(3), 6);
        assert_eq!(BassScale::FiveLevel.from_wire(6), 3);
        assert_eq!(BassScale::ThreeStage.to_wire(2), 10);
        assert_eq!(BassScale::ThreeStage.from_wire(5), 1);
    }
}
