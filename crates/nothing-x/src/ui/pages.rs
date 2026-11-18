//! Page bodies. Each `page_*` draws from `Y0` downwards and returns its bottom y.
//! Single-line labels go through `fit_text` and cards size themselves from their text,
//! so every language fits.

use egui::{Align, Align2, Color32, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use nothing_protocol::gesture::{self, AncCycle, GestureRecord, GestureSlot};
use nothing_protocol::model::BassScale;
use nothing_protocol::{
    AncMode, AncStrength, BatteryLevel, CustomEq, DeviceState, EarFit, EqPreset, Side, SpatialAudio,
};

use super::app::{
    CW, CaseLevel, Cx, Modal, NothingApp, Page, RestartKind, Target, X0, Y0, ago_text, device_name,
};
use super::i18n::{T, t};
use super::theme::{self, pal};
use super::widgets::{self, ButtonKind, SegItem, fit_text};
use super::{dotmatrix, icons};
use crate::session::{Action, Snapshot};

// ---- labels ---------------------------------------------------------------------------

pub(super) fn preset_label(p: EqPreset) -> &'static str {
    match p {
        EqPreset::DiracOpteo => "Dirac OPTEO",
        EqPreset::Pop => t(T::Pop),
        EqPreset::Rock => t(T::Rock),
        EqPreset::Classical => t(T::Classical),
        EqPreset::Electronic => t(T::Electronic),
        EqPreset::EnhanceVocals => t(T::Vocals),
        EqPreset::Custom => t(T::Custom),
    }
}

fn spatial_label(s: SpatialAudio) -> &'static str {
    match s {
        SpatialAudio::Off => t(T::Off),
        SpatialAudio::HeadTracked => t(T::SpatialHead),
        SpatialAudio::Fixed => t(T::SpatialFixed),
        SpatialAudio::Concert => t(T::SpatialConcert),
        SpatialAudio::Theatre => t(T::SpatialTheatre),
        SpatialAudio::Game => t(T::SpatialGame),
    }
}

fn strength_label(s: AncStrength) -> (&'static str, &'static str) {
    match s {
        AncStrength::High => (t(T::High), t(T::HighDesc)),
        AncStrength::Mid => (t(T::Mid), t(T::MidDesc)),
        AncStrength::Low => (t(T::Low), t(T::LowDesc)),
        AncStrength::Adaptive => (t(T::Adaptive), t(T::AdaptiveDesc)),
    }
}

/// (title, description, icon) of the current noise mode.
fn anc_info(mode: Option<AncMode>) -> (&'static str, &'static str, widgets::IconFn) {
    match mode {
        Some(AncMode::NoiseCancelling(_)) => {
            (t(T::AncNc), t(T::AncNcDesc), icons::noise_cancelling)
        }
        Some(AncMode::Transparency) => (
            t(T::AncTransparency),
            t(T::AncTranspDesc),
            icons::transparency,
        ),
        _ => (t(T::Off), t(T::AncOffDesc), icons::off),
    }
}

fn anc_index(mode: Option<AncMode>) -> Option<usize> {
    match mode {
        Some(AncMode::NoiseCancelling(_)) => Some(0),
        Some(AncMode::Transparency) => Some(1),
        Some(AncMode::Off) => Some(2),
        None => None,
    }
}

fn anc_items() -> [SegItem<'static>; 3] {
    [
        SegItem {
            label: t(T::AncNc),
            icon: Some(icons::noise_cancelling),
        },
        SegItem {
            label: t(T::AncTransparency),
            icon: Some(icons::transparency),
        },
        SegItem {
            label: t(T::Off),
            icon: Some(icons::off),
        },
    ]
}

fn slot_title(slot: &GestureSlot) -> String {
    let base = match slot.id {
        "double_tap" => T::GDoubleTap,
        "triple_tap" => T::GTripleTap,
        "tap_and_hold" => T::GTapHold,
        "double_tap_and_hold" => T::GDoubleTapHold,
        "case_single_press" => T::GSinglePress,
        "case_double_press" | "case_double_press_call" => T::GDoublePress,
        "case_triple_press" | "case_triple_press_call" => T::GTriplePress,
        "case_press_hold" => T::GPressHold,
        _ => T::GRotate,
    };
    if slot.id.ends_with("_call") {
        format!("{} · {}", t(base), t(T::DuringCalls))
    } else {
        t(base).to_string()
    }
}

fn action_label(action: u8) -> &'static str {
    use gesture::action as a;
    match action {
        a::NONE => t(T::ANone),
        a::PLAY_PAUSE => t(T::APlayPause),
        a::ANSWER_CALL => t(T::AAnswer),
        a::SKIP_BACK => t(T::ASkipBack),
        a::SKIP_FORWARD => t(T::ASkipForward),
        a::VOICE_ASSISTANT => t(T::AVoice),
        a::LOW_LAG_TOGGLE => t(T::ALowLag),
        a::VOLUME_UP => t(T::AVolUp),
        a::VOLUME_DOWN => t(T::AVolDown),
        a::VOLUME_CONTROL => t(T::AVolume),
        a::ANSWER_CALL_MUTE => t(T::AAnswerMute),
        a::HANG_UP_DECLINE => t(T::AHangUp),
        x if gesture::is_noise_control(x) => t(T::NavNoise),
        _ => "—",
    }
}

fn cycle_summary(cycle: AncCycle) -> String {
    let mut modes = Vec::new();
    if cycle.noise_cancellation {
        modes.push(t(T::AncNc));
    }
    if cycle.transparency {
        modes.push(t(T::AncTransparency));
    }
    if cycle.off {
        modes.push(t(T::Off));
    }
    modes.join(" / ")
}

fn gesture_value(st: &DeviceState, device: u8, slot: &GestureSlot) -> String {
    match st.gesture_action(device, slot) {
        Some(a) if slot.anc_toggle && gesture::is_noise_control(a) => {
            cycle_summary(AncCycle::from_action(a).unwrap_or_default())
        }
        Some(a) => action_label(a).to_string(),
        None => "—".into(),
    }
}

fn target_name(target: Target) -> &'static str {
    match target {
        Target::Left => t(T::LeftEarbud),
        Target::Right => t(T::RightEarbud),
        Target::Dial => t(T::SmartDial),
    }
}

fn battery_text(b: Option<BatteryLevel>) -> String {
    b.map(|b| format!("{}%", b.percent))
        .unwrap_or_else(|| "--".into())
}

/// Icon in a soft disc, used at the start of feature cards.
fn icon_disc(p: &egui::Painter, c: egui::Pos2, icon: widgets::IconFn) {
    let pl = pal();
    p.circle_filled(c, 24.0, pl.surface_hi);
    icon(p, c, 24.0, pl.text);
}

// ---- pages ----------------------------------------------------------------------------

impl NothingApp {
    pub(super) fn page_overview(&mut self, c: &Cx, snap: &Snapshot, now: f64) -> f32 {
        let pl = pal();
        let p = &c.p;
        let st = &snap.state;
        let caps = st.caps;
        let mut y = Y0;

        // Models this version doesn't fully know: say so up front.
        if st.caps.is_none() && st.model.is_some() {
            let body_h =
                widgets::text_height(p, t(T::UnsupportedBody), theme::regular(12.5), CW - 80.0);
            let banner = c.rect(X0, y, CW, body_h + 52.0);
            p.rect_filled(banner, 16.0, pl.warn.gamma_multiply(0.14));
            icons::warning(p, banner.left_top() + vec2(30.0, 26.0), 20.0, pl.warn);
            fit_text(
                p,
                banner.left_top() + vec2(52.0, 26.0),
                Align2::LEFT_CENTER,
                t(T::UnsupportedTitle),
                theme::medium(14.0),
                CW - 80.0,
                pl.text,
            );
            widgets::text_wrapped(
                p,
                banner.left_top() + vec2(52.0, 40.0),
                CW - 80.0,
                t(T::UnsupportedBody),
                theme::regular(12.5),
                pl.text_dim,
                Align::LEFT,
            );
            y += banner.height() + 20.0;
        }

        // Hero: left bud, case, right bud, with dot-matrix battery levels.
        let mid = X0 + CW / 2.0;
        let case = self.case_level(snap);
        let case_live = matches!(case, CaseLevel::Live(_));
        let cols = [
            (Target::Left, mid - 224.0),
            (Target::Dial, mid),
            (Target::Right, mid + 224.0),
        ];
        let mut hero_bottom = y + 240.0;
        for (target, x) in cols {
            let is_case = target == Target::Dial;
            let tex = self.images.as_ref().and_then(|i| match target {
                Target::Left => i.left.clone(),
                Target::Right => i.right.clone(),
                Target::Dial => i.case.clone(),
            });
            let (level, present, label) = match target {
                Target::Left => (st.left, st.left.is_some(), t(T::Left).to_uppercase()),
                Target::Right => (st.right, st.right.is_some(), t(T::Right).to_uppercase()),
                Target::Dial => match case {
                    CaseLevel::Live(b) => (Some(b), true, t(T::Case).to_uppercase()),
                    CaseLevel::Remembered { percent, age_secs } => (
                        Some(BatteryLevel {
                            percent,
                            charging: false,
                        }),
                        false,
                        format!("{} · {}", t(T::Case).to_uppercase(), ago_text(age_secs)),
                    ),
                    CaseLevel::Unknown => (None, false, t(T::Case).to_uppercase()),
                },
            };
            let size = if is_case {
                vec2(190.0, 120.0)
            } else {
                vec2(150.0, 160.0)
            };
            let area = Rect::from_center_size(c.at(x, y + 92.0), size);
            if let Some(tex) = tex {
                let tint = Color32::WHITE.gamma_multiply(if present { 1.0 } else { 0.4 });
                let drawn = tex.paint_fit(p, area, tint);
                let clickable = !is_case || caps.is_some_and(|c| c.smart_dial);
                if clickable && widgets::hit(c.ui, drawn, ("hero", format!("{target:?}"))).clicked()
                {
                    self.target = target;
                    self.go(Page::Controls, now);
                }
            }
            let color = match level {
                Some(_) if !present => pl.text_faint,
                Some(b) if b.percent <= 15 && !b.charging => pl.accent,
                Some(_) => pl.text,
                None => pl.text_faint,
            };
            let digits = level
                .map(|b| format!("{}%", b.percent))
                .unwrap_or_else(|| "--".into());
            dotmatrix::draw(
                p,
                c.at(x, y + 202.0),
                Align2::CENTER_CENTER,
                18.0,
                &digits,
                color,
                false,
            );
            let r = fit_text(
                p,
                c.at(x, y + 230.0),
                Align2::CENTER_CENTER,
                &label,
                theme::medium(11.0),
                200.0,
                pl.text_faint,
            );
            if level.is_some_and(|b| b.charging) && present {
                icons::bolt(p, pos2(r.right() + 10.0, r.center().y), 11.0, pl.good);
            }
            if is_case && !case_live {
                let h = widgets::text_box(
                    p,
                    c.at(x, y + 244.0),
                    260.0,
                    32.0,
                    t(T::CaseHint),
                    theme::regular(11.0),
                    pl.text_faint,
                    Align::Center,
                );
                hero_bottom = hero_bottom.max(y + 244.0 + h);
            }
        }
        y = hero_bottom + 20.0;

        // Noise control.
        let nc = matches!(st.anc, Some(AncMode::NoiseCancelling(_)));
        let strengths = caps.map(|c| c.available_strengths()).unwrap_or_default();
        let show_strength = nc && strengths.len() > 1;
        let (title, desc, _) = anc_info(st.anc);
        let text_w = CW - 24.0 - 420.0 - 24.0 - 16.0;
        let desc_h = widgets::text_height(p, desc, theme::regular(12.0), text_w);
        let top_h = (64.0 + desc_h + 16.0).max(104.0);
        let h = c.anim(
            "ov-nc-h",
            if show_strength { top_h + 64.0 } else { top_h },
            0.22,
        );
        let card = c.rect(X0, y, CW, h);
        widgets::card(p, card);
        fit_text(
            p,
            c.at(X0 + 24.0, y + 30.0),
            Align2::LEFT_CENTER,
            t(T::NavNoise),
            theme::medium(16.0),
            text_w,
            pl.text,
        );
        fit_text(
            p,
            c.at(X0 + 24.0, y + 52.0),
            Align2::LEFT_CENTER,
            title,
            theme::regular(13.0),
            text_w,
            pl.text_dim,
        );
        widgets::text_wrapped(
            p,
            c.at(X0 + 24.0, y + 64.0),
            text_w,
            desc,
            theme::regular(12.0),
            pl.text_faint,
            Align::LEFT,
        );
        let seg = c.rect(X0 + CW - 24.0 - 420.0, y + 20.0, 420.0, 64.0);
        if let Some(i) = widgets::segmented(c.ui, p, seg, "ov-anc", &anc_items(), anc_index(st.anc))
        {
            self.set_anc_index(i, st);
        }
        let alpha = ((h - top_h) / 64.0).clamp(0.0, 1.0);
        if alpha > 0.01 {
            let fp = c.faded(alpha);
            let items: Vec<SegItem> = strengths
                .iter()
                .map(|s| SegItem {
                    label: strength_label(*s).0,
                    icon: None,
                })
                .collect();
            let sel = strengths
                .iter()
                .position(|s| Some(AncMode::NoiseCancelling(*s)) == st.anc);
            let r = c.rect(X0 + CW - 24.0 - 420.0, y + top_h, 420.0, 42.0);
            fit_text(
                &fp,
                c.at(X0 + 24.0, y + top_h + 21.0),
                Align2::LEFT_CENTER,
                t(T::Intensity),
                theme::medium(13.0),
                text_w,
                pl.text_dim,
            );
            if let Some(i) = widgets::segmented(c.ui, &fp, r, "ov-strength", &items, sel) {
                self.act(Action::SetAnc(AncMode::NoiseCancelling(strengths[i])));
            }
        }
        y += h + 16.0;

        // Quick tiles.
        let tw = (CW - 36.0) / 4.0;
        let th = 112.0;
        let bass_on = st.ultra_bass_enabled.unwrap_or(false);
        let bass_level = st.ultra_bass_level.unwrap_or(1);
        let low_lag = st.low_latency.unwrap_or(false);
        let tiles: [(widgets::IconFn, &str, String, Option<bool>, Page); 4] = [
            (
                icons::sound,
                t(T::Equaliser),
                st.eq_preset.map(preset_label).unwrap_or("—").to_string(),
                None,
                Page::Sound,
            ),
            (
                icons::speaker,
                t(T::UltraBass),
                if bass_on {
                    format!("{} {bass_level}", t(T::Level))
                } else {
                    t(T::Off).to_string()
                },
                caps.and_then(|c| c.bass).map(|_| bass_on),
                Page::Sound,
            ),
            (
                icons::bolt,
                t(T::LowLag),
                if low_lag { t(T::On) } else { t(T::Off) }.to_string(),
                Some(low_lag),
                Page::Settings,
            ),
            (
                icons::find,
                t(T::NavFind),
                t(T::TapToRing).to_string(),
                None,
                Page::Find,
            ),
        ];
        for (i, (icon, title, value, toggle, page)) in tiles.into_iter().enumerate() {
            let r = c.rect(X0 + i as f32 * (tw + 12.0), y, tw, th);
            let resp = widgets::hit(c.ui, r, ("tile", i));
            let hover =
                c.ui.ctx()
                    .animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
            p.rect_filled(
                r,
                18.0,
                widgets::lerp_color(pl.surface, pl.surface_hi, hover),
            );
            icon(p, r.left_top() + vec2(28.0, 30.0), 20.0, pl.text);
            fit_text(
                p,
                r.left_top() + vec2(18.0, 70.0),
                Align2::LEFT_CENTER,
                title,
                theme::medium(13.5),
                tw - 30.0,
                pl.text,
            );
            fit_text(
                p,
                r.left_top() + vec2(18.0, 92.0),
                Align2::LEFT_CENTER,
                &value,
                theme::regular(12.5),
                tw - 30.0,
                pl.text_dim,
            );
            let mut toggled = false;
            if let Some(on) = toggle
                && widgets::toggle(
                    c.ui,
                    p,
                    r.right_top() + vec2(-16.0, 30.0),
                    ("tile-toggle", i),
                    on,
                    true,
                )
                .clicked()
            {
                toggled = true;
                match i {
                    1 => self.act(Action::SetUltraBass {
                        enabled: !on,
                        level: bass_level.max(1),
                    }),
                    _ => self.act(Action::SetLowLatency(!on)),
                }
            }
            if resp.clicked() && !toggled {
                self.go(page, now);
            }
        }
        y + th
    }

    fn set_anc_index(&self, i: usize, st: &DeviceState) {
        let mode = match i {
            0 => AncMode::NoiseCancelling(st.anc_strength.unwrap_or(AncStrength::High)),
            1 => AncMode::Transparency,
            _ => AncMode::Off,
        };
        self.act(Action::SetAnc(mode));
    }

    pub(super) fn page_noise(&mut self, c: &Cx, snap: &Snapshot, now: f64) -> f32 {
        let pl = pal();
        let p = &c.p;
        let st = &snap.state;
        let mut y = Y0;
        if let Some(i) = widgets::segmented(
            c.ui,
            p,
            c.rect(X0, y, CW, 84.0),
            "noise-anc",
            &anc_items(),
            anc_index(st.anc),
        ) {
            self.set_anc_index(i, st);
        }
        y += 100.0;

        let (title, desc, icon) = anc_info(st.anc);
        let text_w = CW - 120.0;
        let desc_h = widgets::text_height(p, desc, theme::regular(13.0), text_w);
        let card = c.rect(X0, y, CW, (desc_h + 62.0).max(88.0));
        widgets::card(p, card);
        icon_disc(p, card.left_center() + vec2(48.0, 0.0), icon);
        let top = card.center().y - (22.0 + desc_h) / 2.0;
        fit_text(
            p,
            pos2(card.left() + 92.0, top + 9.0),
            Align2::LEFT_CENTER,
            title,
            theme::medium(16.0),
            text_w,
            pl.text,
        );
        widgets::text_wrapped(
            p,
            pos2(card.left() + 92.0, top + 22.0),
            text_w,
            desc,
            theme::regular(13.0),
            pl.text_dim,
            Align::LEFT,
        );
        y += card.height() + 20.0;

        let strengths = st.caps.map(|c| c.available_strengths()).unwrap_or_default();
        if matches!(st.anc, Some(AncMode::NoiseCancelling(_))) && strengths.len() > 1 {
            widgets::section_label(p, c.at(X0 + 4.0, y), t(T::Intensity));
            y += 24.0;
            let card = c.rect(X0, y, CW, strengths.len() as f32 * 70.0 + 12.0);
            widgets::card(p, card);
            let mut ry = y + 6.0;
            for s in strengths {
                let (name, desc) = strength_label(s);
                let selected = st.anc == Some(AncMode::NoiseCancelling(s));
                let r = c.rect(X0 + 6.0, ry, CW - 12.0, 66.0);
                if widgets::radio_row(
                    c.ui,
                    p,
                    r,
                    ("strength", format!("{s:?}")),
                    name,
                    Some(desc),
                    selected,
                    true,
                )
                .clicked()
                    && !selected
                {
                    self.act(Action::SetAnc(AncMode::NoiseCancelling(s)));
                }
                ry += 70.0;
            }
            y += card.height() + 24.0;
        }

        let h = widgets::text_wrapped(
            p,
            c.at(X0 + 4.0, y),
            CW - 8.0,
            t(T::NoiseTip),
            theme::regular(12.5),
            pl.text_faint,
            Align::LEFT,
        );
        y += h + 10.0;
        let link = widgets::link(
            c.ui,
            p,
            c.at(X0 + 4.0, y + 8.0),
            Align2::LEFT_CENTER,
            "noise-to-controls",
            t(T::NavControls),
            pl.text_dim,
        );
        icons::chevron(
            p,
            pos2(link.rect.right() + 6.0, link.rect.center().y),
            9.0,
            false,
            pl.text_dim,
        );
        if link.clicked() {
            self.go(Page::Controls, now);
        }
        y + 20.0
    }

    pub(super) fn page_sound(&mut self, c: &Cx, snap: &Snapshot) -> f32 {
        let pl = pal();
        let p = &c.p;
        let st = &snap.state;
        let caps = st.caps;
        let mut y = Y0;

        widgets::section_label(p, c.at(X0 + 4.0, y), t(T::Equaliser));
        y += 24.0;

        // Dirac OPTEO.
        let blocked = st.dirac_blocked();
        let selected = st.eq_preset == Some(EqPreset::DiracOpteo);
        let card = c.rect(X0, y, CW, 88.0);
        let resp = widgets::hit(c.ui, card, "dirac");
        let hover =
            c.ui.ctx()
                .animate_bool_with_time(resp.id.with("h"), resp.hovered() && !blocked, 0.12);
        p.rect_filled(
            card,
            18.0,
            widgets::lerp_color(pl.surface, pl.surface_hi, hover),
        );
        if selected {
            p.rect_stroke(card, 18.0, Stroke::new(1.5, pl.inverse), StrokeKind::Inside);
        }
        let alpha = if blocked { 0.45 } else { 1.0 };
        let tr = p.text(
            c.at(X0 + 24.0, y + 32.0),
            Align2::LEFT_CENTER,
            "Dirac OPTEO",
            theme::medium(17.0),
            pl.text.gamma_multiply(alpha),
        );
        let badge = p.layout_no_wrap(
            t(T::Recommended).to_uppercase(),
            theme::medium(9.5),
            pl.text_dim,
        );
        let br = Rect::from_min_size(
            pos2(tr.right() + 10.0, tr.center().y - 9.0),
            badge.size() + vec2(14.0, 8.0),
        );
        p.rect_filled(br, 9.0, pl.surface_hi);
        p.galley(br.min + vec2(7.0, 4.0), badge, pl.text_dim);
        let (desc, color) = if blocked {
            (t(T::DiracLdac), pl.accent)
        } else {
            (t(T::DiracDesc), pl.text_dim)
        };
        fit_text(
            p,
            c.at(X0 + 24.0, y + 58.0),
            Align2::LEFT_CENTER,
            desc,
            theme::regular(13.0),
            CW - 100.0,
            color,
        );
        widgets::radio(p, c.at(X0 + CW - 34.0, y + 44.0), selected);
        if resp.clicked() && !selected {
            if blocked {
                self.modal = Modal::Message {
                    title: T::AudioQuality,
                    body: T::DiracLdac,
                };
            } else {
                self.act(Action::SetEqPreset(EqPreset::DiracOpteo));
            }
        }
        y += 100.0;

        // Presets.
        let presets = [
            EqPreset::Pop,
            EqPreset::Rock,
            EqPreset::Classical,
            EqPreset::Electronic,
            EqPreset::EnhanceVocals,
            EqPreset::Custom,
        ];
        let w = (CW - 24.0) / 3.0;
        for (i, preset) in presets.iter().enumerate() {
            let r = c.rect(
                X0 + (i % 3) as f32 * (w + 12.0),
                y + (i / 3) as f32 * 64.0,
                w,
                52.0,
            );
            let resp = widgets::hit(c.ui, r, ("preset", i));
            let on = st.eq_preset == Some(*preset);
            let hover =
                c.ui.ctx()
                    .animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
            let sel = c.anim_bool(("preset-sel", i), on);
            let bg = widgets::lerp_color(
                widgets::lerp_color(pl.surface, pl.surface_hi, hover),
                pl.inverse,
                sel,
            );
            let fg = widgets::lerp_color(pl.text, pl.on_inverse, sel);
            p.rect_filled(r, 16.0, bg);
            fit_text(
                p,
                r.center(),
                Align2::CENTER_CENTER,
                preset_label(*preset),
                theme::medium(14.5),
                w - 24.0,
                fg,
            );
            if resp.clicked() && !on {
                self.act(Action::SetEqPreset(*preset));
            }
        }
        y += 128.0;

        // Custom editor (expands when Custom is selected).
        let custom = st.eq_preset == Some(EqPreset::Custom);
        let full = 272.0;
        let eh = c.anim("eq-editor-h", if custom { full } else { 0.0 }, 0.25);
        if eh > 1.0 {
            let area = c.rect(X0, y, CW, eh);
            let mut ep = p.with_clip_rect(area.intersect(p.clip_rect()));
            ep.multiply_opacity((eh / full).clamp(0.0, 1.0));
            let ec = Cx {
                ui: c.ui,
                p: ep,
                o: c.o,
                screen: c.screen,
            };
            self.eq_editor(&ec, c.rect(X0, y, CW, full), st, custom);
            y += eh + 24.0;
        } else {
            let h = widgets::text_wrapped(
                p,
                c.at(X0 + 4.0, y),
                CW,
                t(T::CustomHint),
                theme::regular(12.5),
                pl.text_faint,
                Align::LEFT,
            );
            y += h + 24.0;
        }

        // Ultra Bass.
        if let Some(scale) = caps.and_then(|c| c.bass) {
            widgets::section_label(p, c.at(X0 + 4.0, y), t(T::UltraBass));
            y += 24.0;
            let on = st.ultra_bass_enabled.unwrap_or(false);
            let level = st.ultra_bass_level.unwrap_or(1).max(1);
            let bh = c.anim("bass-h", if on { 150.0 } else { 88.0 }, 0.22);
            let card = c.rect(X0, y, CW, bh);
            widgets::card(p, card);
            icon_disc(p, c.at(X0 + 46.0, y + 44.0), icons::speaker);
            let text_w = CW - 86.0 - 90.0;
            fit_text(
                p,
                c.at(X0 + 86.0, y + 32.0),
                Align2::LEFT_CENTER,
                t(T::UltraBass),
                theme::medium(16.0),
                text_w,
                pl.text,
            );
            fit_text(
                p,
                c.at(X0 + 86.0, y + 56.0),
                Align2::LEFT_CENTER,
                t(T::UltraBassDesc),
                theme::regular(13.0),
                text_w,
                pl.text_dim,
            );
            if widgets::toggle(
                c.ui,
                p,
                c.at(X0 + CW - 24.0, y + 44.0),
                "bass-toggle",
                on,
                st.ultra_bass_enabled.is_some(),
            )
            .clicked()
            {
                self.act(Action::SetUltraBass {
                    enabled: !on,
                    level,
                });
            }
            let alpha = ((bh - 88.0) / 62.0).clamp(0.0, 1.0);
            if alpha > 0.01 {
                let fp = c.faded(alpha);
                let levels: Vec<u8> = scale.levels().filter(|l| *l >= 1).collect();
                let labels: Vec<String> = levels
                    .iter()
                    .map(|l| format!("{} {l}", t(T::Level)))
                    .collect();
                let items: Vec<SegItem> = labels
                    .iter()
                    .map(|l| SegItem {
                        label: l,
                        icon: None,
                    })
                    .collect();
                let sel = levels.iter().position(|l| *l == level);
                let r = c.rect(X0 + 24.0, y + 92.0, CW - 48.0, 40.0);
                if let Some(i) = widgets::segmented(c.ui, &fp, r, "bass-levels", &items, sel) {
                    self.act(Action::SetUltraBass {
                        enabled: scale != BassScale::ThreeStage || levels[i] > 0,
                        level: levels[i],
                    });
                }
            }
            y += bh + 24.0;
        }

        // Spatial audio.
        let modes = caps.map(|c| c.available_spatial()).unwrap_or_default();
        if modes.len() > 1 {
            widgets::section_label(p, c.at(X0 + 4.0, y), t(T::SpatialAudio));
            y += 24.0;
            let seg_w = 280.0;
            let text_w = CW - 24.0 - seg_w - 40.0;
            let desc_h = widgets::text_height(p, t(T::SpatialDesc), theme::regular(12.5), text_w);
            let card = c.rect(X0, y, CW, (desc_h + 62.0).max(92.0));
            widgets::card(p, card);
            let top = card.center().y - (22.0 + desc_h) / 2.0;
            fit_text(
                p,
                pos2(card.left() + 24.0, top + 9.0),
                Align2::LEFT_CENTER,
                t(T::SpatialAudio),
                theme::medium(16.0),
                text_w,
                pl.text,
            );
            widgets::text_wrapped(
                p,
                pos2(card.left() + 24.0, top + 22.0),
                text_w,
                t(T::SpatialDesc),
                theme::regular(12.5),
                pl.text_dim,
                Align::LEFT,
            );
            let items: Vec<SegItem> = modes
                .iter()
                .map(|m| SegItem {
                    label: spatial_label(*m),
                    icon: None,
                })
                .collect();
            let sel = modes.iter().position(|m| Some(*m) == st.spatial);
            let r = Rect::from_min_size(
                pos2(card.right() - 24.0 - seg_w, card.center().y - 22.0),
                vec2(seg_w, 44.0),
            );
            if let Some(i) = widgets::segmented(c.ui, p, r, "spatial", &items, sel) {
                self.act(Action::SetSpatialAudio(modes[i]));
            }
            y += card.height();
        }
        y
    }

    /// 3-band custom EQ: a smooth curve with draggable handles.
    fn eq_editor(&mut self, c: &Cx, rect: Rect, st: &DeviceState, interactive: bool) {
        let pl = pal();
        let p = &c.p;
        widgets::card(p, rect);
        let reset_w = widgets::button_width(p, t(T::Reset)).max(110.0);
        let text_w = rect.width() - 48.0 - reset_w - 16.0;
        fit_text(
            p,
            rect.left_top() + vec2(24.0, 32.0),
            Align2::LEFT_CENTER,
            t(T::Custom),
            theme::medium(16.0),
            text_w,
            pl.text,
        );
        fit_text(
            p,
            rect.left_top() + vec2(24.0, 54.0),
            Align2::LEFT_CENTER,
            t(T::DragHint),
            theme::regular(12.5),
            text_w,
            pl.text_dim,
        );
        let reset = Rect::from_min_size(
            rect.right_top() + vec2(-24.0 - reset_w, 22.0),
            vec2(reset_w, 36.0),
        );
        if interactive
            && widgets::button(
                c.ui,
                p,
                reset,
                "eq-reset",
                t(T::Reset),
                ButtonKind::Secondary,
                true,
            )
            .clicked()
        {
            self.eq_drag = None;
            self.act(Action::SetCustomEq(CustomEq::default()));
        }

        let g = Rect::from_min_size(
            rect.left_top() + vec2(56.0, 92.0),
            vec2(rect.width() - 112.0, 110.0),
        );
        let mut values = st.custom_eq.unwrap_or_default().as_array();
        if let Some((i, v)) = self.eq_drag {
            values[i] = v;
        }
        let xs = [
            g.left() + g.width() / 6.0,
            g.center().x,
            g.left() + g.width() * 5.0 / 6.0,
        ];
        let y_of = |v: f32| g.center().y - v / 6.0 * g.height() / 2.0;

        for (v, label) in [(6.0, "+6"), (0.0, "0"), (-6.0, "-6")] {
            let yy = y_of(v);
            let stroke = Stroke::new(
                1.0,
                if v == 0.0 {
                    pl.stroke
                } else {
                    pl.stroke.gamma_multiply(0.5)
                },
            );
            p.line_segment([pos2(g.left(), yy), pos2(g.right(), yy)], stroke);
            p.text(
                pos2(g.left() - 12.0, yy),
                Align2::RIGHT_CENTER,
                label,
                theme::regular(10.5),
                pl.text_faint,
            );
        }

        // Smooth curve (cosine interpolation, flat beyond the outer bands).
        let sample = |x: f32| -> f32 {
            if x <= xs[0] {
                return values[0];
            }
            if x >= xs[2] {
                return values[2];
            }
            let i = if x < xs[1] { 0 } else { 1 };
            let tt = (x - xs[i]) / (xs[i + 1] - xs[i]);
            let k = (1.0 - (tt * std::f32::consts::PI).cos()) / 2.0;
            values[i] * (1.0 - k) + values[i + 1] * k
        };
        let n = 64;
        let pts: Vec<egui::Pos2> = (0..=n)
            .map(|i| {
                let x = g.left() + g.width() * i as f32 / n as f32;
                pos2(x, y_of(sample(x)))
            })
            .collect();
        let base = y_of(0.0);
        let mut mesh = egui::Mesh::default();
        for q in &pts {
            let strength = ((base - q.y).abs() / (g.height() / 2.0)).clamp(0.0, 1.0);
            mesh.colored_vertex(*q, pl.accent.gamma_multiply(0.12 + 0.28 * strength));
            mesh.colored_vertex(pos2(q.x, base), pl.accent.gamma_multiply(0.02));
        }
        for i in 0..n as u32 {
            let b = i * 2;
            mesh.add_triangle(b, b + 1, b + 2);
            mesh.add_triangle(b + 1, b + 3, b + 2);
        }
        p.add(mesh);
        p.add(egui::Shape::line(pts, Stroke::new(2.0, pl.text)));

        for (i, band) in [T::Bass, T::MidBand, T::Treble].iter().enumerate() {
            let handle = pos2(xs[i], y_of(values[i]));
            let column = Rect::from_center_size(
                pos2(xs[i], g.center().y),
                vec2(g.width() / 3.0, g.height() + 36.0),
            );
            let resp = if interactive {
                widgets::hit_sense(c.ui, column, ("eq-band", i), Sense::click_and_drag())
            } else {
                c.ui.interact(
                    Rect::NOTHING,
                    egui::Id::unique(("eq-band", i)),
                    Sense::hover(),
                )
            };
            let active = resp.hovered() || resp.dragged();
            if (resp.dragged() || resp.clicked())
                && interactive
                && let Some(pos) = resp.interact_pointer_pos()
            {
                let v = ((g.center().y - pos.y) / (g.height() / 2.0) * 6.0)
                    .round()
                    .clamp(-6.0, 6.0);
                self.eq_drag = Some((i, v));
            }
            if (resp.drag_stopped() || resp.clicked())
                && interactive
                && let Some((di, v)) = self.eq_drag.take()
            {
                let mut eq = st.custom_eq.unwrap_or_default().as_array();
                eq[di] = v;
                self.act(Action::SetCustomEq(CustomEq::from_array(eq)));
            }
            let r = if active { 11.0 } else { 9.0 };
            p.circle_filled(handle, r, if active { pl.inverse } else { pl.surface });
            p.circle_stroke(handle, r, Stroke::new(2.0, pl.inverse));
            let v = values[i] as i32;
            let value = if v > 0 {
                format!("+{v} dB")
            } else {
                format!("{v} dB")
            };
            fit_text(
                p,
                pos2(xs[i], g.bottom() + 26.0),
                Align2::CENTER_CENTER,
                t(*band),
                theme::regular(12.5),
                g.width() / 3.0 - 8.0,
                pl.text_dim,
            );
            p.text(
                pos2(xs[i], g.bottom() + 46.0),
                Align2::CENTER_CENTER,
                value,
                theme::medium(14.0),
                pl.text,
            );
        }
    }

    pub(super) fn page_controls(&mut self, c: &Cx, snap: &Snapshot) -> f32 {
        let pl = pal();
        let p = &c.p;
        let st = &snap.state;
        let has_dial = st.caps.is_some_and(|c| c.smart_dial);
        if !has_dial && self.target == Target::Dial {
            self.target = Target::Left;
        }
        let mut y = Y0;
        let mut targets = vec![Target::Left, Target::Right];
        if has_dial {
            targets.push(Target::Dial);
        }
        let items: Vec<SegItem> = targets
            .iter()
            .map(|tg| SegItem {
                label: target_name(*tg),
                icon: None,
            })
            .collect();
        let sel = targets.iter().position(|tg| *tg == self.target);
        let seg_w = if has_dial { 540.0 } else { 360.0 };
        if let Some(i) = widgets::segmented(
            c.ui,
            p,
            c.rect(X0, y, seg_w, 44.0),
            "ctl-target",
            &items,
            sel,
        ) {
            self.target = targets[i];
        }
        y += 64.0;

        let tex = self.images.as_ref().and_then(|i| match self.target {
            Target::Left => i.left.clone(),
            Target::Right => i.right.clone(),
            Target::Dial => i.case.clone(),
        });
        let img_h = if self.target == Target::Dial {
            200.0
        } else {
            300.0
        };
        if let Some(tex) = tex {
            let alpha = c.anim_bool(("ctl-img", format!("{:?}", self.target)), true);
            tex.paint_fit(
                &c.faded(alpha),
                c.rect(X0, y + 10.0, 230.0, img_h),
                Color32::WHITE,
            );
        }

        let xr = X0 + 254.0;
        let wr = CW - 254.0;
        let device = self.target.device();
        let slots: Vec<GestureSlot> = match self.target {
            Target::Dial => gesture::CASE_SLOTS.to_vec(),
            _ => gesture::BUD_SLOTS.to_vec(),
        };
        let mut ry = y;
        for slot in slots {
            let r = c.rect(xr, ry, wr, 68.0);
            let resp = widgets::hit(c.ui, r, ("gesture-row", slot.id));
            let hover =
                c.ui.ctx()
                    .animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
            p.rect_filled(
                r,
                16.0,
                widgets::lerp_color(pl.surface, pl.surface_hi, hover),
            );
            fit_text(
                p,
                r.left_top() + vec2(20.0, 24.0),
                Align2::LEFT_CENTER,
                &slot_title(&slot),
                theme::medium(14.5),
                wr - 64.0,
                pl.text,
            );
            fit_text(
                p,
                r.left_top() + vec2(20.0, 46.0),
                Align2::LEFT_CENTER,
                &gesture_value(st, device, &slot),
                theme::regular(13.0),
                wr - 64.0,
                pl.text_dim,
            );
            icons::chevron(
                p,
                pos2(r.right() - 22.0, r.center().y),
                12.0,
                false,
                pl.text_faint,
            );
            if resp.clicked() {
                self.modal = Modal::Gesture {
                    target: self.target,
                    slot,
                };
            }
            ry += 76.0;
        }
        ry += 8.0;
        widgets::section_label(p, c.at(xr + 4.0, ry), t(T::FixedTitle));
        ry += 24.0;
        let notes: &[T] = if self.target == Target::Dial {
            &[T::FixedLock]
        } else {
            &[T::FixedAnswer, T::FixedHangUp]
        };
        for note in notes {
            p.circle_filled(c.at(xr + 9.0, ry + 9.0), 2.5, pl.text_faint);
            let h = widgets::text_wrapped(
                p,
                c.at(xr + 22.0, ry),
                wr - 30.0,
                t(*note),
                theme::regular(13.0),
                pl.text_dim,
                Align::LEFT,
            );
            ry += h + 8.0;
        }
        ry.max(y + img_h + 20.0)
    }

    pub(super) fn modal_gesture(
        &mut self,
        c: &Cx,
        card: Rect,
        st: &DeviceState,
        target: Target,
        slot: GestureSlot,
    ) -> bool {
        let pl = pal();
        let p = &c.p;
        fit_text(
            p,
            c.at(28.0, 38.0),
            Align2::LEFT_CENTER,
            &slot_title(&slot),
            theme::medium(18.0),
            card.width() - 56.0,
            pl.text,
        );
        fit_text(
            p,
            c.at(28.0, 62.0),
            Align2::LEFT_CENTER,
            target_name(target),
            theme::regular(13.0),
            card.width() - 56.0,
            pl.text_dim,
        );
        let device = target.device();
        let current = st.gesture_action(device, &slot);
        let is_noise = current.is_some_and(gesture::is_noise_control);
        let mut y = 86.0;
        let mut close = false;
        for (i, option) in slot.options.iter().enumerate() {
            let noise_opt = slot.anc_toggle && option.action == gesture::action::NOISE_CONTROL_ALL;
            let selected = current
                .is_some_and(|a| a == option.action || (noise_opt && gesture::is_noise_control(a)));
            let r = c.rect(12.0, y, card.width() - 24.0, 52.0);
            if widgets::radio_row(
                c.ui,
                p,
                r,
                ("gesture-opt", i),
                action_label(option.action),
                None,
                selected,
                true,
            )
            .clicked()
                && !selected
            {
                let action = if noise_opt {
                    AncCycle::default().to_action().unwrap_or(option.action)
                } else {
                    option.action
                };
                self.set_gesture(target, &slot, action);
                close = !noise_opt;
            }
            y += 56.0;
        }
        if slot.anc_toggle && is_noise {
            y += 10.0;
            widgets::section_label(p, c.at(28.0, y), t(T::CycleTitle));
            y += 24.0;
            let cycle = st.anc_cycle(device, &slot);
            let items = [
                (t(T::AncNc), cycle.noise_cancellation),
                (t(T::AncTransparency), cycle.transparency),
                (t(T::Off), cycle.off),
            ];
            for (i, (label, checked)) in items.iter().enumerate() {
                let r = c.rect(12.0, y, card.width() - 24.0, 40.0);
                let resp = widgets::hit(c.ui, r, ("cycle", i));
                if resp.hovered() {
                    p.rect_filled(r, 12.0, pl.surface_hi.gamma_multiply(0.7));
                }
                widgets::checkbox(p, r.left_center() + vec2(26.0, 0.0), *checked);
                fit_text(
                    p,
                    r.left_center() + vec2(48.0, 0.0),
                    Align2::LEFT_CENTER,
                    label,
                    theme::medium(14.0),
                    r.width() - 60.0,
                    pl.text,
                );
                if resp.clicked() {
                    let mut next = cycle;
                    match i {
                        0 => next.noise_cancellation = !next.noise_cancellation,
                        1 => next.transparency = !next.transparency,
                        _ => next.off = !next.off,
                    }
                    if let Some(action) = next.to_action() {
                        // The cycle is shared by both earbuds.
                        let targets: &[Target] = if target == Target::Dial {
                            &[Target::Dial]
                        } else {
                            &[Target::Left, Target::Right]
                        };
                        for tg in targets {
                            self.set_gesture(*tg, &slot, action);
                        }
                    }
                }
                y += 44.0;
            }
            widgets::text_box(
                p,
                c.at(28.0, y + 4.0),
                card.width() - 56.0,
                34.0,
                t(T::CycleHint),
                theme::regular(12.0),
                pl.text_faint,
                Align::LEFT,
            );
        }
        let done_w = widgets::button_width(p, t(T::Done)).max(110.0);
        let done = c.rect(
            card.width() - 28.0 - done_w,
            card.height() - 60.0,
            done_w,
            40.0,
        );
        close
            | widgets::button(
                c.ui,
                p,
                done,
                "gesture-done",
                t(T::Done),
                ButtonKind::Primary,
                true,
            )
            .clicked()
    }

    fn set_gesture(&self, target: Target, slot: &GestureSlot, action: u8) {
        self.act(Action::SetGesture(GestureRecord {
            device: target.device(),
            common: slot.send_common(),
            kind: slot.kind,
            action,
        }));
    }

    pub(super) fn page_find(&mut self, c: &Cx, snap: &Snapshot, now: f64) -> f32 {
        let pl = pal();
        let p = &c.p;
        let st = &snap.state;
        let mut y = Y0;
        let warn_h = widgets::text_height(p, t(T::FindWarning), theme::medium(13.0), CW - 80.0);
        let banner = c.rect(X0, y, CW, (warn_h + 30.0).max(56.0));
        p.rect_filled(banner, 16.0, pl.warn.gamma_multiply(0.14));
        icons::warning(p, banner.left_center() + vec2(30.0, 0.0), 20.0, pl.warn);
        widgets::text_wrapped(
            p,
            pos2(banner.left() + 52.0, banner.center().y - warn_h / 2.0),
            CW - 80.0,
            t(T::FindWarning),
            theme::medium(13.0),
            pl.text,
            Align::LEFT,
        );
        y += banner.height() + 20.0;

        let w = (CW - 20.0) / 2.0;
        for (i, side) in [Side::Left, Side::Right].iter().enumerate() {
            let x = X0 + i as f32 * (w + 20.0);
            let card = c.rect(x, y, w, 404.0);
            widgets::card(p, card);
            let (ringing, level, tex, name) = match side {
                Side::Left => (
                    st.ringing_left,
                    st.left,
                    self.images.as_ref().and_then(|i| i.left.clone()),
                    t(T::LeftEarbud),
                ),
                Side::Right => (
                    st.ringing_right,
                    st.right,
                    self.images.as_ref().and_then(|i| i.right.clone()),
                    t(T::RightEarbud),
                ),
            };
            let center = c.at(x + w / 2.0, y + 148.0);
            if ringing {
                for k in 0..3 {
                    let phase = ((now * 0.7) as f32 + k as f32 / 3.0).fract();
                    p.circle_stroke(
                        center,
                        70.0 + phase * 70.0,
                        Stroke::new(2.0, pl.accent.gamma_multiply((1.0 - phase) * 0.6)),
                    );
                }
                c.ui.ctx().request_repaint();
            }
            if let Some(tex) = tex {
                let tint = Color32::WHITE.gamma_multiply(if level.is_some() { 1.0 } else { 0.4 });
                tex.paint_fit(p, Rect::from_center_size(center, vec2(170.0, 190.0)), tint);
            }
            fit_text(
                p,
                c.at(x + w / 2.0, y + 282.0),
                Align2::CENTER_CENTER,
                name,
                theme::medium(16.0),
                w - 32.0,
                pl.text,
            );
            let (status, color) = if ringing {
                (t(T::Ringing).to_string(), pl.accent)
            } else if level.is_some() {
                (
                    format!("{} · {}", t(T::Connected), battery_text(level)),
                    pl.text_dim,
                )
            } else {
                (t(T::NotConnected).to_string(), pl.text_faint)
            };
            fit_text(
                p,
                c.at(x + w / 2.0, y + 306.0),
                Align2::CENTER_CENTER,
                &status,
                theme::regular(12.5),
                w - 32.0,
                color,
            );
            let b = c.rect(x + 24.0, y + 334.0, w - 48.0, 44.0);
            let (label, kind) = if ringing {
                (t(T::Stop), ButtonKind::Accent)
            } else {
                (t(T::TapToRing), ButtonKind::Primary)
            };
            if widgets::button(c.ui, p, b, ("ring", i), label, kind, true).clicked() {
                self.act(Action::Ring {
                    side: *side,
                    on: !ringing,
                });
            }
        }
        y + 404.0
    }

    pub(super) fn page_fit(&mut self, c: &Cx, snap: &Snapshot, now: f64) -> f32 {
        let pl = pal();
        let p = &c.p;
        let st = &snap.state;
        let y = Y0;
        let card = c.rect(X0, y, CW, 476.0);
        widgets::card(p, card);
        let running = snap.ear_fit_running && st.ear_fit.is_none();
        let mid = X0 + CW / 2.0;
        for (side, dx) in [(Side::Left, -150.0), (Side::Right, 150.0)] {
            let x = mid + dx;
            let tex = self.images.as_ref().and_then(|i| {
                if side == Side::Left {
                    i.left.clone()
                } else {
                    i.right.clone()
                }
            });
            if let Some(tex) = tex {
                tex.paint_fit(
                    p,
                    Rect::from_center_size(c.at(x, y + 124.0), vec2(150.0, 170.0)),
                    Color32::WHITE,
                );
            }
            let result = st
                .ear_fit
                .map(|(l, r)| if side == Side::Left { l } else { r });
            let (dot, label) = match result {
                None if running => (pl.text_faint, "…"),
                None => (pl.text_faint, "—"),
                Some(EarFit::Good) => (pl.good, t(T::FitGood)),
                Some(EarFit::Adjust) => (pl.warn, t(T::FitAdjust)),
                Some(_) => (pl.accent, t(T::FitNotDetected)),
            };
            let side_name = if side == Side::Left {
                t(T::Left)
            } else {
                t(T::Right)
            };
            p.text(
                c.at(x, y + 226.0),
                Align2::CENTER_CENTER,
                side_name.to_uppercase(),
                theme::medium(11.0),
                pl.text_faint,
            );
            let max_label = 240.0;
            let galley_w = p
                .layout_no_wrap(label.to_string(), theme::medium(13.0), pl.text)
                .size()
                .x
                .min(max_label);
            let chip = Rect::from_center_size(c.at(x, y + 254.0), vec2(galley_w + 44.0, 32.0));
            p.rect_filled(chip, 16.0, pl.surface_hi);
            p.circle_filled(chip.left_center() + vec2(16.0, 0.0), 5.0, dot);
            fit_text(
                p,
                chip.left_center() + vec2(28.0, 0.0),
                Align2::LEFT_CENTER,
                label,
                theme::medium(13.0),
                max_label,
                pl.text,
            );
        }

        let msg_y = y + 300.0;
        if running {
            icons::spinner(p, c.at(mid, msg_y + 22.0), 16.0, now, pl.text);
            fit_text(
                p,
                c.at(mid, msg_y + 58.0),
                Align2::CENTER_CENTER,
                t(T::FitRunning),
                theme::regular(14.0),
                CW - 48.0,
                pl.text_dim,
            );
            c.ui.ctx().request_repaint();
        } else {
            match st.ear_fit {
                None => {
                    let steps = [T::FitStep1, T::FitStep2, T::FitStep3];
                    let widest = steps
                        .iter()
                        .map(|s| {
                            p.layout_no_wrap(t(*s).to_string(), theme::regular(13.5), pl.text)
                                .size()
                                .x
                        })
                        .fold(0.0f32, f32::max)
                        .min(CW - 120.0);
                    let x0 = mid - (widest + 32.0) / 2.0;
                    for (i, step) in steps.iter().enumerate() {
                        let ly = msg_y + i as f32 * 28.0 + 4.0;
                        let nc = pos2(c.at(x0, 0.0).x + 11.0, c.at(0.0, ly).y);
                        p.circle_filled(nc, 11.0, pl.surface_hi);
                        p.text(
                            nc,
                            Align2::CENTER_CENTER,
                            (i + 1).to_string(),
                            theme::medium(12.0),
                            pl.text,
                        );
                        fit_text(
                            p,
                            nc + vec2(22.0, 0.0),
                            Align2::LEFT_CENTER,
                            t(*step),
                            theme::regular(13.5),
                            CW - 120.0,
                            pl.text_dim,
                        );
                    }
                }
                Some((EarFit::Good, EarFit::Good)) => {
                    fit_text(
                        p,
                        c.at(mid, msg_y + 32.0),
                        Align2::CENTER_CENTER,
                        t(T::FitPerfect),
                        theme::medium(17.0),
                        CW - 48.0,
                        pl.text,
                    );
                }
                Some((l, r)) => {
                    let msg =
                        if matches!(l, EarFit::NotDetected) || matches!(r, EarFit::NotDetected) {
                            t(T::FitCheckBoth)
                        } else {
                            t(T::FitAdvice)
                        };
                    widgets::text_box(
                        p,
                        c.at(mid, msg_y + 6.0),
                        CW - 120.0,
                        90.0,
                        msg,
                        theme::regular(14.0),
                        pl.text_dim,
                        Align::Center,
                    );
                }
            }
        }
        let label = if st.ear_fit.is_some() {
            t(T::FitAgain)
        } else {
            t(T::FitStart)
        };
        let bw = widgets::button_width(p, label).max(220.0);
        let b = c.rect(mid - bw / 2.0, y + 406.0, bw, 44.0);
        if widgets::button(
            c.ui,
            p,
            b,
            "fit-start",
            label,
            ButtonKind::Primary,
            !running,
        )
        .clicked()
        {
            self.act(Action::StartEarFitTest);
        }
        y + 476.0
    }

    pub(super) fn page_settings(&mut self, c: &Cx, snap: &Snapshot) -> f32 {
        let pl = pal();
        let p = &c.p;
        let st = &snap.state;
        let caps = st.caps;
        let mut y = Y0;

        // Earbuds.
        widgets::section_label(p, c.at(X0 + 4.0, y), t(T::SecEarbuds));
        y += 24.0;
        let mut rows: Vec<(T, T, Option<bool>, u8)> = Vec::new();
        if caps.is_none_or(|c| c.in_ear_detection) {
            rows.push((T::InEar, T::InEarDesc, st.in_ear_detection, 0));
        }
        rows.push((T::LowLag, T::LowLagDesc, st.low_latency, 1));
        let heights: Vec<f32> = rows
            .iter()
            .map(|(ti, d, _, _)| widgets::row_height(p, CW, t(*ti), Some(t(*d)), 90.0, 76.0))
            .collect();
        let card = c.rect(X0, y, CW, heights.iter().sum());
        widgets::card(p, card);
        let mut ry = y;
        for (i, (title, desc, value, kind)) in rows.iter().enumerate() {
            let r = c.rect(X0, ry, CW, heights[i]);
            if i > 0 {
                widgets::divider(p, r.left() + 20.0, r.right() - 20.0, r.top());
            }
            widgets::row_text(p, r, t(*title), Some(t(*desc)), 90.0);
            let on = value.unwrap_or(false);
            if widgets::toggle(
                c.ui,
                p,
                r.right_center() - vec2(24.0, 0.0),
                ("set-toggle", *kind),
                on,
                value.is_some(),
            )
            .clicked()
            {
                match kind {
                    0 => self.act(Action::SetInEarDetection(!on)),
                    _ => self.act(Action::SetLowLatency(!on)),
                }
            }
            ry += heights[i];
        }
        y += card.height() + 28.0;

        // Connection.
        let dual = caps.is_some_and(|c| c.dual_connection);
        let codecs = caps.map(|c| c.available_codecs()).unwrap_or_default();
        if dual || codecs.len() > 1 {
            widgets::section_label(p, c.at(X0 + 4.0, y), t(T::SecConnection));
            y += 24.0;
            let dual_on = st.dual_enabled.unwrap_or(false);
            let devices = if dual && dual_on {
                st.dual_devices.len().min(4)
            } else {
                0
            };
            let reboot = caps.is_some_and(|c| c.dual_connection_reboot);
            let dual_desc = if reboot {
                format!("{} {}", t(T::DualDesc), t(T::RestartNote))
            } else {
                t(T::DualDesc).to_string()
            };
            let dual_h = widgets::row_height(p, CW, t(T::DualConn), Some(&dual_desc), 90.0, 84.0);
            let codec_h = widgets::row_height(
                p,
                CW,
                t(T::AudioQuality),
                Some(t(T::AudioQualityDesc)),
                250.0,
                88.0,
            );
            let mut h = 0.0;
            if dual {
                h += dual_h
                    + if devices > 0 {
                        32.0 + devices as f32 * 56.0 + 8.0
                    } else {
                        0.0
                    };
            }
            if codecs.len() > 1 {
                h += codec_h;
            }
            let card = c.rect(X0, y, CW, h);
            widgets::card(p, card);
            let mut ry = y;
            if dual {
                let r = c.rect(X0, ry, CW, dual_h);
                widgets::row_text(p, r, t(T::DualConn), Some(&dual_desc), 90.0);
                if widgets::toggle(
                    c.ui,
                    p,
                    r.right_center() - vec2(24.0, 0.0),
                    "dual-toggle",
                    dual_on,
                    st.dual_enabled.is_some(),
                )
                .clicked()
                {
                    if reboot {
                        self.modal = Modal::Restart(RestartKind::Dual(!dual_on));
                    } else {
                        self.act(Action::SetDualEnabled(!dual_on));
                    }
                }
                ry += dual_h;
                if devices > 0 {
                    let connected = st.dual_devices.iter().filter(|d| d.connected).count();
                    fit_text(
                        p,
                        c.at(X0 + 20.0, ry + 14.0),
                        Align2::LEFT_CENTER,
                        &t(T::DevicesConnected).replace("{n}", &connected.to_string()),
                        theme::regular(12.0),
                        CW - 40.0,
                        pl.text_faint,
                    );
                    ry += 32.0;
                    for d in st.dual_devices.iter().take(4) {
                        let r = c.rect(X0 + 12.0, ry, CW - 24.0, 52.0);
                        p.rect_filled(r, 12.0, pl.surface_hi.gamma_multiply(0.6));
                        let name = if d.name.is_empty() {
                            t(T::UnknownDevice).to_string()
                        } else {
                            d.name.clone()
                        };
                        let name = if d.is_self {
                            format!("{name} · {}", t(T::ThisMac))
                        } else {
                            name
                        };
                        fit_text(
                            p,
                            r.left_top() + vec2(16.0, 17.0),
                            Align2::LEFT_CENTER,
                            &name,
                            theme::medium(13.5),
                            r.width() - 100.0,
                            pl.text,
                        );
                        let status = if d.connected {
                            t(T::Connected)
                        } else {
                            t(T::NotConnected)
                        };
                        fit_text(
                            p,
                            r.left_top() + vec2(16.0, 36.0),
                            Align2::LEFT_CENTER,
                            status,
                            theme::regular(12.0),
                            r.width() - 100.0,
                            pl.text_dim,
                        );
                        let disabled = (!d.connected && connected >= 2) || d.is_self;
                        if widgets::toggle(
                            c.ui,
                            p,
                            r.right_center() - vec2(14.0, 0.0),
                            ("dual-dev", d.mac),
                            d.connected,
                            !disabled,
                        )
                        .clicked()
                        {
                            self.act(Action::DualConnect {
                                mac: d.mac,
                                connect: !d.connected,
                            });
                        }
                        ry += 56.0;
                    }
                    ry += 8.0;
                }
            }
            if codecs.len() > 1 {
                let r = c.rect(X0, ry, CW, codec_h);
                if dual {
                    widgets::divider(p, r.left() + 20.0, r.right() - 20.0, r.top());
                }
                widgets::row_text(
                    p,
                    r,
                    t(T::AudioQuality),
                    Some(t(T::AudioQualityDesc)),
                    250.0,
                );
                let items: Vec<SegItem> = codecs
                    .iter()
                    .map(|k| SegItem {
                        label: k.label(),
                        icon: None,
                    })
                    .collect();
                let sel = codecs.iter().position(|k| Some(*k) == st.codec);
                let seg = Rect::from_min_size(
                    pos2(r.right() - 24.0 - 200.0, r.center().y - 20.0),
                    vec2(200.0, 40.0),
                );
                if let Some(i) = widgets::segmented(c.ui, p, seg, "codec", &items, sel) {
                    self.modal = Modal::Restart(RestartKind::Codec(codecs[i]));
                }
            }
            y += card.height() + 28.0;
        }

        // App.
        widgets::section_label(p, c.at(X0 + 4.0, y), t(T::SecApp));
        y += 24.0;
        let card = c.rect(X0, y, CW, 64.0 + 80.0);
        widgets::card(p, card);
        let lang_row = c.rect(X0, y, CW, 64.0);
        let resp = widgets::hit(c.ui, lang_row, "lang-row");
        if resp.hovered() {
            p.rect_filled(
                lang_row.shrink(4.0),
                14.0,
                pl.surface_hi.gamma_multiply(0.7),
            );
        }
        icons::globe(p, lang_row.left_center() + vec2(32.0, 0.0), 18.0, pl.text);
        let label_r = fit_text(
            p,
            lang_row.left_center() + vec2(54.0, 0.0),
            Align2::LEFT_CENTER,
            t(T::Language),
            theme::medium(14.5),
            220.0,
            pl.text,
        );
        let current = match self.lang_pref {
            Some(l) => l.native_name().to_string(),
            None => format!("{} ({})", t(T::System), self.current_lang().native_name()),
        };
        let value_max = lang_row.right() - 44.0 - label_r.right() - 24.0;
        fit_text(
            p,
            lang_row.right_center() - vec2(44.0, 0.0),
            Align2::RIGHT_CENTER,
            &current,
            theme::regular(13.5),
            value_max,
            pl.text_dim,
        );
        icons::chevron(
            p,
            lang_row.right_center() - vec2(24.0, 0.0),
            12.0,
            false,
            pl.text_faint,
        );
        if resp.clicked() {
            self.modal = Modal::Language;
        }
        let ar = c.rect(X0, y + 64.0, CW, 80.0);
        widgets::divider(p, ar.left() + 20.0, ar.right() - 20.0, ar.top());
        widgets::row_text(p, ar, t(T::Appearance), None, 330.0);
        let prefs = [
            egui::ThemePreference::System,
            egui::ThemePreference::Dark,
            egui::ThemePreference::Light,
        ];
        let items = [
            SegItem {
                label: t(T::System),
                icon: None,
            },
            SegItem {
                label: t(T::Dark),
                icon: None,
            },
            SegItem {
                label: t(T::Light),
                icon: None,
            },
        ];
        let sel = prefs.iter().position(|x| *x == self.theme_pref);
        let seg = Rect::from_min_size(
            pos2(ar.right() - 24.0 - 300.0, ar.center().y - 20.0),
            vec2(300.0, 40.0),
        );
        if let Some(i) = widgets::segmented(c.ui, p, seg, "appearance", &items, sel) {
            self.theme_pref = prefs[i];
            c.ui.ctx().set_theme(prefs[i]);
        }
        y += card.height() + 28.0;

        // About.
        widgets::section_label(p, c.at(X0 + 4.0, y), t(T::SecAbout));
        y += 24.0;
        let model = st
            .model
            .map(|m| format!("{} ({} · {})", m.name, m.id_hex(), m.config_id()))
            .unwrap_or_else(|| "—".into());
        let address = snap
            .device
            .as_ref()
            .map(|d| d.address.to_uppercase().replace('-', ":"))
            .unwrap_or_default();
        let info = [
            (t(T::DeviceName), device_name(snap)),
            (t(T::Model), model),
            (
                t(T::Firmware),
                st.firmware.clone().unwrap_or_else(|| "—".into()),
            ),
            (
                t(T::Serial),
                st.serial.clone().unwrap_or_else(|| "—".into()),
            ),
            (t(T::Address), address),
        ];
        let card = c.rect(X0, y, CW, info.len() as f32 * 48.0 + 76.0);
        widgets::card(p, card);
        for (i, (label, value)) in info.iter().enumerate() {
            let ry = y + 8.0 + i as f32 * 48.0;
            if i > 0 {
                widgets::divider(
                    p,
                    c.at(X0 + 20.0, 0.0).x,
                    c.at(X0 + CW - 20.0, 0.0).x,
                    c.at(0.0, ry).y,
                );
            }
            let lr = fit_text(
                p,
                c.at(X0 + 20.0, ry + 24.0),
                Align2::LEFT_CENTER,
                label,
                theme::regular(13.5),
                220.0,
                pl.text_dim,
            );
            fit_text(
                p,
                c.at(X0 + CW - 20.0, ry + 24.0),
                Align2::RIGHT_CENTER,
                value,
                theme::medium(13.5),
                c.at(X0 + CW - 20.0, 0.0).x - lr.right() - 24.0,
                pl.text,
            );
        }
        let by = y + 8.0 + info.len() as f32 * 48.0 + 12.0;
        let mut bx = X0 + 20.0;
        for (i, (label, kind)) in [
            (t(T::Refresh), ButtonKind::Secondary),
            (t(T::ViewLogs), ButtonKind::Secondary),
            (t(T::Disconnect), ButtonKind::Danger),
        ]
        .iter()
        .enumerate()
        {
            let w = widgets::button_width(p, label);
            if widgets::button(
                c.ui,
                p,
                c.rect(bx, by, w, 40.0),
                ("about-btn", i),
                label,
                *kind,
                true,
            )
            .clicked()
            {
                match i {
                    0 => self.act(Action::Refresh),
                    1 => self.modal = Modal::Logs,
                    _ => self.session.disconnect(),
                }
            }
            bx += w + 10.0;
        }
        y += card.height() + 28.0;

        // Credits.
        widgets::section_label(p, c.at(X0 + 4.0, y), t(T::SecCredits));
        y += 24.0;
        let lines = [
            t(T::CreditFirst),
            t(T::CreditProtocol),
            t(T::CreditTrademarks),
        ];
        let text_h: f32 = lines
            .iter()
            .map(|l| widgets::text_height(p, l, theme::regular(13.0), CW - 40.0) + 10.0)
            .sum();
        let card = c.rect(X0, y, CW, text_h + 84.0);
        widgets::card(p, card);
        let mut ty = y + 20.0;
        for (i, line) in lines.iter().enumerate() {
            let color = if i == 0 { pl.text } else { pl.text_dim };
            ty += widgets::text_wrapped(
                p,
                c.at(X0 + 20.0, ty),
                CW - 40.0,
                line,
                theme::regular(13.0),
                color,
                Align::LEFT,
            ) + 10.0;
        }
        let mut bx = X0 + 20.0;
        let links = [
            (
                "eldandelion/nothing-x-macos",
                "https://github.com/eldandelion/nothing-x-macos",
            ),
            (
                "radiance-project/ear-web",
                "https://github.com/radiance-project/ear-web",
            ),
        ];
        for (i, (label, url)) in links.iter().enumerate() {
            let w = widgets::button_width(p, label);
            if widgets::button(
                c.ui,
                p,
                c.rect(bx, ty + 4.0, w, 38.0),
                ("credit-link", i),
                label,
                ButtonKind::Secondary,
                true,
            )
            .clicked()
            {
                let _ = std::process::Command::new("open").arg(url).spawn();
            }
            bx += w + 10.0;
        }
        y += card.height() + 20.0;
        let h = widgets::text_wrapped(
            p,
            c.at(X0 + 4.0, y),
            CW - 8.0,
            t(T::Disclaimer),
            theme::regular(11.5),
            pl.text_faint,
            Align::LEFT,
        );
        y + h
    }
}
