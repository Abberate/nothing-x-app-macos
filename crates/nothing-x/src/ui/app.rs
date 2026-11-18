//! Application shell: sidebar (device switcher, batteries, navigation), page routing,
//! dialogs, toasts and the connection screen. Page bodies live in `pages.rs`.

use egui::{
    Align, Align2, Color32, Id, Key, Painter, Pos2, Rect, Sense, Stroke, ThemePreference, Ui,
    ViewportCommand, pos2, vec2,
};
use nothing_bt::PairedDevice;
use nothing_protocol::gesture::GestureSlot;
use nothing_protocol::{Codec, DeviceState};

use super::assets::BudImages;
use super::i18n::{self, Lang, T, t};
use super::theme::{self, pal};
use super::widgets::{self, ButtonKind, IconFn};
use super::{dotmatrix, icons};
use crate::session::{Action, NoticeKind, Phase, SessionHandle, Snapshot};

pub const APP_NAME: &str = "Nothing X App";

/// Design canvas width in points; the canvas is scaled to the window width.
pub(super) const W: f32 = 1040.0;
pub(super) const SIDEBAR: f32 = 280.0;
pub(super) const X0: f32 = 324.0;
pub(super) const CW: f32 = 672.0;
pub(super) const HEADER_H: f32 = 128.0;
pub(super) const Y0: f32 = HEADER_H + 12.0;

const KEY_LAST_DEVICE: &str = "last_device";
const KEY_LANG: &str = "lang";
const KEY_THEME: &str = "theme";
const KEY_CASE: &str = "case_memory";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub(super) enum Page {
    Overview,
    Noise,
    Sound,
    Controls,
    Find,
    Fit,
    Settings,
}

impl Page {
    pub const ALL: [Page; 7] = [
        Page::Overview,
        Page::Noise,
        Page::Sound,
        Page::Controls,
        Page::Find,
        Page::Fit,
        Page::Settings,
    ];

    fn index(self) -> usize {
        Page::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }

    fn title(self) -> T {
        match self {
            Page::Overview => T::NavOverview,
            Page::Noise => T::NavNoise,
            Page::Sound => T::NavSound,
            Page::Controls => T::NavControls,
            Page::Find => T::NavFind,
            Page::Fit => T::NavFit,
            Page::Settings => T::NavSettings,
        }
    }

    fn subtitle(self) -> Option<T> {
        match self {
            Page::Overview => None,
            Page::Noise => Some(T::NoiseSubtitle),
            Page::Sound => Some(T::SoundSubtitle),
            Page::Controls => Some(T::ControlsSubtitle),
            Page::Find => Some(T::FindSubtitle),
            Page::Fit => Some(T::FitSubtitle),
            Page::Settings => Some(T::SettingsSubtitle),
        }
    }

    fn icon(self) -> IconFn {
        match self {
            Page::Overview => icons::overview,
            Page::Noise => icons::noise_cancelling,
            Page::Sound => icons::sound,
            Page::Controls => icons::controls,
            Page::Find => icons::find,
            Page::Fit => icons::fit,
            Page::Settings => icons::settings,
        }
    }
}

/// Which part of the device the Controls page edits.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub(super) enum Target {
    Left,
    Right,
    Dial,
}

impl Target {
    pub fn device(self) -> u8 {
        match self {
            Target::Left => nothing_protocol::DEVICE_LEFT,
            Target::Right => nothing_protocol::DEVICE_RIGHT,
            Target::Dial => nothing_protocol::DEVICE_CASE,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum Modal {
    None,
    Gesture { target: Target, slot: GestureSlot },
    Restart(RestartKind),
    Message { title: T, body: T },
    Devices,
    Language,
    Logs,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum RestartKind {
    Codec(Codec),
    Dual(bool),
}

pub struct NothingApp {
    pub(super) session: SessionHandle,
    pub(super) images: Option<BudImages>,
    pub(super) page: Page,
    page_changed_at: f64,
    pub(super) target: Target,
    pub(super) modal: Modal,
    paired: Vec<PairedDevice>,
    paired_at: Option<f64>,
    show_all: bool,
    last_device: Option<String>,
    auto_connect_done: bool,
    pub(super) eq_drag: Option<(usize, f32)>,
    screenshot: Option<(std::path::PathBuf, u32)>,
    scroll: [f32; 7],
    content_bottom: [f32; 7],
    toast: Option<(String, f64)>,
    last_notice: u64,
    pub(super) lang_pref: Option<Lang>,
    system_lang: Lang,
    pub(super) theme_pref: ThemePreference,
    /// Last case level seen per device address: (percent, unix seconds).
    case_memory: std::collections::HashMap<String, (u8, u64)>,
    /// False in demo mode or when settings are forced from the environment, so test
    /// runs never overwrite the user's preferences.
    persist: bool,
    /// Simulated device (no real Bluetooth listing either).
    demo: bool,
}

impl NothingApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let ctx = cc.egui_ctx.clone();
        let demo =
            std::env::args().any(|a| a == "--demo") || std::env::var_os("NOTHINGX_DEMO").is_some();
        let session = if demo {
            SessionHandle::spawn_demo(move || ctx.request_repaint())
        } else {
            SessionHandle::spawn(move || ctx.request_repaint())
        };
        let storage = cc.storage;
        let get = |k: &str| storage.and_then(|s| s.get_string(k));
        let lang_pref = std::env::var("NOTHINGX_LANG")
            .ok()
            .or_else(|| get(KEY_LANG))
            .and_then(|c| Lang::from_code(&c));
        let theme_pref = match std::env::var("NOTHINGX_THEME")
            .ok()
            .or_else(|| get(KEY_THEME))
            .as_deref()
        {
            Some("dark") => ThemePreference::Dark,
            Some("light") => ThemePreference::Light,
            _ => ThemePreference::System,
        };
        cc.egui_ctx.set_theme(theme_pref);
        let frames = std::env::var("NOTHINGX_FRAMES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(45);
        let screenshot = std::env::var_os("NOTHINGX_SCREENSHOT").map(|p| (p.into(), frames));
        let mut app = Self {
            session,
            images: None,
            page: Page::Overview,
            page_changed_at: 0.0,
            target: Target::Left,
            modal: Modal::None,
            paired: Vec::new(),
            paired_at: None,
            show_all: false,
            last_device: get(KEY_LAST_DEVICE),
            auto_connect_done: demo || std::env::var_os("NOTHINGX_NO_AUTOCONNECT").is_some(),
            eq_drag: None,
            screenshot,
            scroll: [0.0; 7],
            content_bottom: [0.0; 7],
            toast: None,
            last_notice: 0,
            lang_pref,
            system_lang: Lang::system(),
            theme_pref,
            case_memory: get(KEY_CASE)
                .map(|s| parse_case_memory(&s))
                .unwrap_or_default(),
            demo,
            persist: !demo
                && [
                    "NOTHINGX_LANG",
                    "NOTHINGX_THEME",
                    "NOTHINGX_SCENE",
                    "NOTHINGX_SCREENSHOT",
                ]
                .iter()
                .all(|k| std::env::var_os(k).is_none()),
        };
        if let Ok(scene) = std::env::var("NOTHINGX_SCENE") {
            app.apply_scene(&scene);
        }
        app
    }

    /// Opens a given screen directly (demo mode, visual checks).
    fn apply_scene(&mut self, scene: &str) {
        match scene {
            "noise" => self.page = Page::Noise,
            "sound" => self.page = Page::Sound,
            "custom" => {
                self.page = Page::Sound;
                self.act(Action::SetEqPreset(nothing_protocol::EqPreset::Custom));
            }
            "controls" => self.page = Page::Controls,
            "dial" => {
                self.page = Page::Controls;
                self.target = Target::Dial;
            }
            "gesture" => {
                self.page = Page::Controls;
                self.modal = Modal::Gesture {
                    target: Target::Left,
                    slot: nothing_protocol::gesture::BUD_SLOTS[2],
                };
            }
            "find" => self.page = Page::Find,
            "fit" => self.page = Page::Fit,
            "fit-run" => {
                self.page = Page::Fit;
                self.act(Action::StartEarFitTest);
            }
            "settings" => self.page = Page::Settings,
            "about" => {
                self.page = Page::Settings;
                self.scroll[Page::Settings.index()] = 10_000.0;
            }
            "restart" => {
                self.page = Page::Settings;
                self.modal = Modal::Restart(RestartKind::Codec(Codec::Ldac));
            }
            "language" => {
                self.page = Page::Settings;
                self.modal = Modal::Language;
            }
            "devices" => self.modal = Modal::Devices,
            "case-away" => {
                self.case_memory.insert(
                    "00-00-00-00-00-00".into(),
                    (62, unix_now().saturating_sub(12 * 60)),
                );
            }
            _ => {}
        }
    }

    pub(super) fn act(&self, a: Action) {
        self.session.act(a);
    }

    pub(super) fn go(&mut self, page: Page, now: f64) {
        if self.page != page {
            self.page = page;
            self.page_changed_at = now;
        }
    }

    pub(super) fn current_lang(&self) -> Lang {
        self.lang_pref.unwrap_or(self.system_lang)
    }

    pub(super) fn show_toast(&mut self, text: &str, now: f64) {
        self.toast = Some((text.to_string(), now));
    }

    pub(super) fn connect_to(&mut self, d: &PairedDevice) {
        self.session.connect(d.clone());
        self.page = Page::Overview;
        self.scroll = [0.0; 7];
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some((path, frames)) = &mut self.screenshot else {
            return;
        };
        if *frames == 0 {
            let path = path.clone();
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::ScreenshotCallback::new(
                move |img| {
                    let [w, h] = img.size;
                    let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                    if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, rgba) {
                        let _ = buf.save(&path);
                    }
                    std::process::exit(0);
                },
            )));
            *frames = u32::MAX;
        } else if *frames != u32::MAX {
            *frames -= 1;
        }
        ctx.request_repaint();
    }

    fn refresh_paired(&mut self, now: f64) {
        if self.paired_at.is_some_and(|t| now - t < 3.0) {
            return;
        }
        self.paired_at = Some(now);
        self.paired = if self.demo {
            self.session.snapshot().device.into_iter().collect()
        } else {
            nothing_bt::paired_devices()
        };
        if !self.auto_connect_done {
            self.auto_connect_done = true;
            let candidates: Vec<&PairedDevice> = self
                .paired
                .iter()
                .filter(|d| d.looks_like_nothing())
                .collect();
            let pick = self
                .last_device
                .as_ref()
                .and_then(|a| self.paired.iter().find(|d| &d.address == a))
                .or(if candidates.len() == 1 {
                    Some(candidates[0])
                } else {
                    None
                })
                .cloned();
            if let Some(d) = pick {
                self.connect_to(&d);
            }
        }
    }

    /// Paired devices shown in lists, connected ones first.
    fn device_list(&self) -> Vec<PairedDevice> {
        let mut list: Vec<PairedDevice> = self
            .paired
            .iter()
            .filter(|d| self.show_all || d.looks_like_nothing())
            .cloned()
            .collect();
        list.sort_by_key(|d| (!d.connected, d.name.to_lowercase()));
        list
    }

    fn ensure_images(&mut self, ctx: &egui::Context, state: &DeviceState) {
        let key = BudImages::resolve_key(state.model.map(|m| m.image));
        if self.images.as_ref().map(|i| i.key.as_str()) != Some(key) {
            self.images = Some(BudImages::load(ctx, key));
        }
    }

    fn apply_zoom(ctx: &egui::Context, screen: Rect) {
        let z = ctx.zoom_factor();
        let raw = screen.size() * z;
        let target = (raw.x / W).min(raw.y / 600.0).clamp(0.7, 2.0);
        if (target - z).abs() > 0.01 {
            ctx.set_zoom_factor(target);
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context, now: f64) {
        let keys = [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
        ];
        let mut go = None;
        let mut escape = false;
        ctx.input(|i| {
            if i.modifiers.command {
                for (k, page) in keys.iter().zip(Page::ALL) {
                    if i.key_pressed(*k) {
                        go = Some(page);
                    }
                }
            }
            escape = i.key_pressed(Key::Escape);
        });
        if let Some(p) = go
            && matches!(self.modal, Modal::None)
        {
            self.go(p, now);
        }
        if escape && !matches!(self.modal, Modal::None) {
            self.close_modal();
        }
    }

    pub(super) fn close_modal(&mut self) {
        self.modal = Modal::None;
    }
}

/// Name of the connected earbuds as macOS knows them (the user can rename them).
pub(super) fn device_name(snap: &Snapshot) -> String {
    snap.device
        .as_ref()
        .map(|d| d.name.trim().to_string())
        .filter(|n| !n.is_empty())
        .or_else(|| snap.state.model.map(|m| m.name.to_string()))
        .unwrap_or_else(|| APP_NAME.to_string())
}

impl eframe::App for NothingApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_screenshot(&ctx);
        theme::set_palette(if ctx.theme() == egui::Theme::Dark {
            theme::DARK
        } else {
            theme::LIGHT
        });
        i18n::set_lang(self.current_lang());

        let screen = ui.max_rect();
        Self::apply_zoom(&ctx, screen);
        let now = ctx.input(|i| i.time);
        let snap = self.session.snapshot();
        self.shortcuts(&ctx, now);

        if let Some(n) = snap.notice
            && n.id != self.last_notice
        {
            self.last_notice = n.id;
            let text = match n.kind {
                NoticeKind::NoAck => t(T::ToastNoAck),
                NoticeKind::Connected => t(T::ToastConnected),
            };
            self.show_toast(text, now);
        }

        ui.painter().rect_filled(screen, 0.0, pal().bg);
        let origin = pos2(
            (screen.center().x - W / 2.0).max(screen.left()),
            screen.top(),
        );
        self.ensure_images(&ctx, &snap.state);
        self.remember_case(&snap);

        let connected_ui = snap.state.model.is_some()
            && matches!(
                snap.phase,
                Phase::Ready | Phase::Initializing | Phase::Rebooting
            );
        if connected_ui {
            if let Some(d) = &snap.device
                && snap.phase == Phase::Ready
                && self.last_device.as_deref() != Some(d.address.as_str())
            {
                self.last_device = Some(d.address.clone());
            }
            if matches!(self.modal, Modal::Devices) {
                self.refresh_paired(now);
            }
        } else {
            self.refresh_paired(now);
            if matches!(
                self.modal,
                Modal::Gesture { .. } | Modal::Restart(_) | Modal::Devices
            ) {
                self.modal = Modal::None;
            }
        }

        let blocked = !matches!(self.modal, Modal::None) || snap.phase == Phase::Rebooting;
        widgets::set_input_blocked(blocked);
        let cx = Cx {
            ui,
            p: ui.painter().clone(),
            o: origin,
            screen,
        };
        if connected_ui {
            self.shell(&cx, &snap, now);
        } else {
            self.connect_screen(&cx, &snap, now);
        }
        widgets::set_clip(None);
        widgets::set_input_blocked(false);

        // Window drag strip (the title bar is transparent).
        let drag = ui.interact(
            Rect::from_min_size(screen.min, vec2(screen.width(), 28.0)),
            Id::unique("window-drag"),
            Sense::drag(),
        );
        if drag.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }

        self.modals(&cx, &snap, now);
        self.toast_view(&cx, now);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if !self.persist {
            return;
        }
        if let Some(a) = &self.last_device {
            storage.set_string(KEY_LAST_DEVICE, a.clone());
        }
        storage.set_string(
            KEY_LANG,
            self.lang_pref
                .map(|l| l.code().to_string())
                .unwrap_or_else(|| "system".into()),
        );
        let theme = match self.theme_pref {
            ThemePreference::Dark => "dark",
            ThemePreference::Light => "light",
            ThemePreference::System => "system",
        };
        storage.set_string(KEY_THEME, theme.into());
        let memory: Vec<String> = self
            .case_memory
            .iter()
            .map(|(a, (p, t))| format!("{a}={p}@{t}"))
            .collect();
        storage.set_string(KEY_CASE, memory.join(";"));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.session.shutdown();
    }
}

/// Drawing context on the design canvas.
pub(super) struct Cx<'a> {
    pub ui: &'a Ui,
    pub p: Painter,
    pub o: Pos2,
    pub screen: Rect,
}

impl Cx<'_> {
    pub fn at(&self, x: f32, y: f32) -> Pos2 {
        self.o + vec2(x, y)
    }
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.at(x, y), vec2(w, h))
    }
    pub fn anim(&self, id: impl std::hash::Hash + std::fmt::Debug, value: f32, secs: f32) -> f32 {
        self.ui
            .ctx()
            .animate_value_with_time(Id::unique(id), value, secs)
    }
    pub fn anim_bool(&self, id: impl std::hash::Hash + std::fmt::Debug, on: bool) -> f32 {
        self.ui
            .ctx()
            .animate_bool_with_time(Id::unique(id), on, 0.2)
    }
    pub fn faded(&self, alpha: f32) -> Painter {
        let mut p = self.p.clone();
        p.multiply_opacity(alpha.clamp(0.0, 1.0));
        p
    }
}

/// Large title: dot-matrix when every character has a glyph, otherwise the UI font.
pub(super) fn title_text(
    p: &Painter,
    pos: Pos2,
    align: Align2,
    height: f32,
    text: &str,
    color: Color32,
    max_w: f32,
) -> Rect {
    let upper = ascii_upper(text);
    if dotmatrix::supports(&upper) {
        // Long names shrink (down to 60 %), then fall back to the UI font.
        let w = dotmatrix::width(&upper, height);
        let h = if w > max_w {
            height * max_w / w
        } else {
            height
        };
        if h >= height * 0.6 {
            return dotmatrix::draw(p, pos, align, h, &upper, color, true);
        }
    }
    widgets::fit_text(
        p,
        pos,
        align,
        text,
        theme::medium(height * 1.15),
        max_w,
        color,
    )
}

/// What to show for the case battery.
#[derive(Clone, Copy, Debug)]
pub(super) enum CaseLevel {
    Live(nothing_protocol::BatteryLevel),
    /// Last level seen while an earbud was in the case.
    Remembered {
        percent: u8,
        age_secs: u64,
    },
    Unknown,
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn parse_case_memory(s: &str) -> std::collections::HashMap<String, (u8, u64)> {
    s.split(';')
        .filter_map(|e| {
            let (addr, rest) = e.split_once('=')?;
            let (pct, at) = rest.split_once('@')?;
            Some((addr.to_string(), (pct.parse().ok()?, at.parse().ok()?)))
        })
        .collect()
}

/// "il y a 12 min", "à l'instant"…
pub(super) fn ago_text(secs: u64) -> String {
    let (key, n) = match secs {
        0..60 => return t(T::JustNow).to_string(),
        60..3600 => (T::MinutesAgo, secs / 60),
        3600..86_400 => (T::HoursAgo, secs / 3600),
        _ => (T::DaysAgo, secs / 86_400),
    };
    t(key).replace("{n}", &n.to_string())
}

impl NothingApp {
    /// Live case level, or the last one remembered for this device.
    pub(super) fn case_level(&self, snap: &Snapshot) -> CaseLevel {
        if let Some(b) = snap.state.case {
            return CaseLevel::Live(b);
        }
        snap.device
            .as_ref()
            .and_then(|d| self.case_memory.get(&d.address))
            .map(|(percent, at)| CaseLevel::Remembered {
                percent: *percent,
                age_secs: unix_now().saturating_sub(*at),
            })
            .unwrap_or(CaseLevel::Unknown)
    }

    fn remember_case(&mut self, snap: &Snapshot) {
        if let (Some(b), Some(d)) = (snap.state.case, &snap.device) {
            self.case_memory
                .insert(d.address.clone(), (b.percent, unix_now()));
        }
    }
}

// ---------------------------------------------------------------------------------------
// Shell: sidebar + page
// ---------------------------------------------------------------------------------------

impl NothingApp {
    fn shell(&mut self, cx: &Cx, snap: &Snapshot, now: f64) {
        let pl = pal();
        let side = Rect::from_min_max(cx.screen.min, pos2(cx.o.x + SIDEBAR, cx.screen.bottom()));
        cx.p.rect_filled(side, 0.0, pl.sidebar);
        cx.p.line_segment(
            [side.right_top(), side.right_bottom()],
            Stroke::new(1.0, pl.stroke),
        );
        self.sidebar(cx, snap, now);

        // Header.
        let fade = ((now - self.page_changed_at) / 0.22).clamp(0.0, 1.0) as f32;
        let fade = 1.0 - (1.0 - fade).powi(3);
        if fade < 1.0 {
            cx.ui.ctx().request_repaint();
        }
        let hp = cx.faded(fade);
        let (title, subtitle) = match self.page {
            Page::Overview => (device_name(snap), overview_subtitle(snap)),
            p => (
                t(p.title()).to_string(),
                p.subtitle().map(|s| t(s).to_string()).unwrap_or_default(),
            ),
        };
        title_text(
            &hp,
            cx.at(X0, 66.0),
            Align2::LEFT_CENTER,
            22.0,
            &title,
            pl.text,
            CW,
        );
        widgets::fit_text(
            &hp,
            cx.at(X0, 100.0),
            Align2::LEFT_CENTER,
            &subtitle,
            theme::regular(14.0),
            CW,
            pl.text_dim,
        );

        // Scrollable content.
        let idx = self.page.index();
        let clip = Rect::from_min_max(
            pos2(cx.o.x + SIDEBAR + 1.0, cx.screen.top() + HEADER_H),
            cx.screen.right_bottom(),
        );
        let visible_bottom = cx.screen.bottom() - cx.o.y;
        let max_scroll = (self.content_bottom[idx] - visible_bottom).max(0.0);
        let hovered = cx
            .ui
            .ctx()
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|p| clip.contains(p));
        if hovered && !widgets::input_blocked() {
            let dy = cx.ui.ctx().input(|i| i.smooth_scroll_delta.y);
            self.scroll[idx] -= dy;
        }
        if self.content_bottom[idx] > 0.0 {
            self.scroll[idx] = self.scroll[idx].clamp(0.0, max_scroll);
        }
        let offset = self.scroll[idx] - (1.0 - fade) * 14.0;

        let mut painter = cx.p.with_clip_rect(clip);
        painter.multiply_opacity(fade);
        let pc = Cx {
            ui: cx.ui,
            p: painter,
            o: pos2(cx.o.x, cx.o.y - offset),
            screen: cx.screen,
        };
        widgets::set_clip(Some(clip));
        let bottom = match self.page {
            Page::Overview => self.page_overview(&pc, snap, now),
            Page::Noise => self.page_noise(&pc, snap, now),
            Page::Sound => self.page_sound(&pc, snap),
            Page::Controls => self.page_controls(&pc, snap),
            Page::Find => self.page_find(&pc, snap, now),
            Page::Fit => self.page_fit(&pc, snap, now),
            Page::Settings => self.page_settings(&pc, snap),
        };
        widgets::set_clip(None);
        self.content_bottom[idx] = bottom + 40.0;

        if max_scroll > 1.0 {
            let track_h = clip.height() - 16.0;
            let thumb_h = (track_h * clip.height() / (clip.height() + max_scroll)).max(30.0);
            let y = clip.top() + 8.0 + (track_h - thumb_h) * self.scroll[idx] / max_scroll;
            cx.p.rect_filled(
                Rect::from_min_size(pos2(clip.right() - 7.0, y), vec2(3.0, thumb_h)),
                2.0,
                pl.text_faint.gamma_multiply(0.5),
            );
        }
        // Soft edge under the header so scrolled content doesn't cut harshly.
        let band = Rect::from_min_size(clip.min, vec2(clip.width(), 14.0));
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(band.left_top(), pl.bg);
        mesh.colored_vertex(band.right_top(), pl.bg);
        mesh.colored_vertex(band.left_bottom(), Color32::TRANSPARENT);
        mesh.colored_vertex(band.right_bottom(), Color32::TRANSPARENT);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(1, 3, 2);
        cx.p.add(mesh);

        if snap.phase == Phase::Initializing {
            let area = Rect::from_min_max(
                pos2(cx.o.x + SIDEBAR + 1.0, cx.screen.top()),
                cx.screen.right_bottom(),
            );
            cx.p.rect_filled(area, 0.0, pl.bg.gamma_multiply(0.94));
            icons::spinner(&cx.p, area.center() - vec2(0.0, 16.0), 26.0, now, pl.text);
            cx.p.text(
                area.center() + vec2(0.0, 30.0),
                Align2::CENTER_CENTER,
                t(T::PhaseSetup),
                theme::regular(14.0),
                pl.text_dim,
            );
            cx.ui.ctx().request_repaint();
        }
    }

    fn sidebar(&mut self, cx: &Cx, snap: &Snapshot, now: f64) {
        let pl = pal();
        let p = &cx.p;
        let st = &snap.state;

        // Device switcher: the earbuds' own name, opens the device list.
        let head = cx.rect(14.0, 40.0, 252.0, 62.0);
        let resp = widgets::hit(cx.ui, head, "device-switcher");
        let hover = cx
            .ui
            .ctx()
            .animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
        p.rect_filled(head, 14.0, pl.surface_hi.gamma_multiply(hover));
        let name = device_name(snap);
        widgets::fit_text(
            p,
            head.left_top() + vec2(14.0, 21.0),
            Align2::LEFT_CENTER,
            &name,
            theme::medium(16.0),
            head.width() - 56.0,
            pl.text,
        );
        let (dot, status) = match snap.phase {
            Phase::Ready | Phase::Initializing => (pl.good, t(T::Connected).to_string()),
            Phase::Rebooting => (pl.warn, t(T::PhaseRebooting).to_string()),
            _ => (pl.text_faint, t(T::PhaseConnecting).to_string()),
        };
        let status = match st.codec {
            Some(c) if snap.phase == Phase::Ready => format!("{status} · {}", c.label()),
            _ => status,
        };
        p.circle_filled(head.left_top() + vec2(18.0, 44.0), 3.5, dot);
        widgets::fit_text(
            p,
            head.left_top() + vec2(28.0, 44.0),
            Align2::LEFT_CENTER,
            &status,
            theme::medium(12.5),
            head.width() - 70.0,
            pl.text_dim,
        );
        icons::chevron_down(
            p,
            pos2(head.right() - 20.0, head.center().y),
            9.0,
            pl.text_dim,
        );
        if resp.clicked() {
            self.paired_at = None;
            self.modal = Modal::Devices;
        }

        // Batteries.
        let (case, case_live) = match self.case_level(snap) {
            CaseLevel::Live(b) => (Some(b), true),
            CaseLevel::Remembered { percent, .. } => (
                Some(nothing_protocol::BatteryLevel {
                    percent,
                    charging: false,
                }),
                false,
            ),
            CaseLevel::Unknown => (None, false),
        };
        let cols = [
            (T::Left, st.left, true),
            (T::Right, st.right, true),
            (T::Case, case, case_live),
        ];
        for (i, (label, level, live)) in cols.iter().enumerate() {
            let x = 28.0 + i as f32 * 80.0;
            widgets::fit_text(
                p,
                cx.at(x, 126.0),
                Align2::LEFT_CENTER,
                &t(*label).to_uppercase(),
                theme::medium(11.0),
                74.0,
                pl.text_faint,
            );
            match level {
                Some(b) => {
                    let r = dotmatrix::draw(
                        p,
                        cx.at(x, 150.0),
                        Align2::LEFT_CENTER,
                        13.0,
                        &b.percent.to_string(),
                        if *live { pl.text } else { pl.text_faint },
                        false,
                    );
                    p.text(
                        pos2(r.right() + 3.0, r.bottom()),
                        Align2::LEFT_BOTTOM,
                        "%",
                        theme::medium(11.0),
                        pl.text_dim,
                    );
                    if b.charging && *live {
                        icons::bolt(p, pos2(r.right() + 20.0, r.center().y), 11.0, pl.good);
                    }
                }
                None => {
                    dotmatrix::draw(
                        p,
                        cx.at(x, 150.0),
                        Align2::LEFT_CENTER,
                        13.0,
                        "--",
                        pl.text_faint,
                        false,
                    );
                }
            }
            widgets::battery_bar(p, cx.rect(x, 168.0, 62.0, 4.0), level.map(|b| b.percent));
        }

        let mut y = 200.0;
        for page in Page::ALL {
            if page == Page::Fit && !st.caps.is_none_or(|c| c.ear_tip_test) {
                continue;
            }
            let r = cx.rect(16.0, y, 248.0, 40.0);
            if widgets::nav_item(
                cx.ui,
                p,
                r,
                ("nav", page),
                page.icon(),
                t(page.title()),
                self.page == page,
            )
            .clicked()
            {
                self.go(page, now);
            }
            y += 44.0;
        }

        p.text(
            pos2(cx.o.x + 28.0, cx.screen.bottom() - 26.0),
            Align2::LEFT_CENTER,
            format!(
                "{APP_NAME} · v{} · {}",
                env!("CARGO_PKG_VERSION"),
                t(T::Unofficial)
            ),
            theme::regular(11.0),
            pl.text_faint,
        );
    }
}

fn overview_subtitle(snap: &Snapshot) -> String {
    let name = device_name(snap);
    let mut parts = Vec::new();
    if let Some(m) = snap.state.model
        && !m.name.eq_ignore_ascii_case(&name)
    {
        parts.push(m.name.to_string());
    }
    if let Some(fw) = &snap.state.firmware {
        parts.push(format!("{} {fw}", t(T::Firmware)));
    }
    parts.join(" · ")
}

/// The dot-matrix font covers ASCII only: map accented letters to their base form.
pub(super) fn ascii_upper(s: &str) -> String {
    s.to_uppercase()
        .chars()
        .map(|c| match c {
            'À' | 'Á' | 'Â' | 'Ä' | 'Ã' => 'A',
            'È' | 'É' | 'Ê' | 'Ë' => 'E',
            'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
            'Ò' | 'Ó' | 'Ô' | 'Ö' | 'Õ' => 'O',
            'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
            'Ç' => 'C',
            'Ñ' => 'N',
            'ß' => 'S',
            '’' => '\'',
            c => c,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------
// Connection screen
// ---------------------------------------------------------------------------------------

impl NothingApp {
    fn connect_screen(&mut self, cx: &Cx, snap: &Snapshot, now: f64) {
        let pl = pal();
        let p = &cx.p;
        let center = W / 2.0;
        let col_x = center - 250.0;
        let mut y = 104.0;

        dotmatrix::draw(
            p,
            cx.at(center, y),
            Align2::CENTER_CENTER,
            28.0,
            "NOTHING X APP",
            pl.text,
            true,
        );
        y += 42.0;
        p.text(
            cx.at(center, y),
            Align2::CENTER_CENTER,
            t(T::Tagline),
            theme::regular(15.0),
            pl.text_dim,
        );
        y += 22.0;
        p.text(
            cx.at(center, y),
            Align2::CENTER_CENTER,
            t(T::Unofficial).to_uppercase(),
            theme::medium(10.5),
            pl.text_faint,
        );
        y += 36.0;

        let target = snap.device.as_ref().map(|d| d.address.clone());
        if let (Some(d), true) = (&snap.device, snap.phase != Phase::Idle) {
            let (label, hint) = match &snap.phase {
                Phase::WaitingForDevice => (T::PhaseWaiting, Some(T::HintWaiting)),
                Phase::Identifying => (T::PhaseIdentifying, None),
                Phase::Connecting => (T::PhaseConnecting, None),
                Phase::Initializing => (T::PhaseSetup, None),
                Phase::Rebooting => (T::PhaseRebooting, None),
                Phase::Retrying { reason, .. } if reason.contains("in use") => {
                    (T::PhaseRetrying, Some(T::HintBusy))
                }
                _ => (T::PhaseRetrying, None),
            };
            let hint_h = hint
                .map(|h| widgets::text_height(p, t(h), theme::regular(13.0), 400.0))
                .unwrap_or(0.0);
            let h = 82.0 + if hint.is_some() { hint_h + 8.0 } else { 0.0 };
            let card = cx.rect(col_x, y, 500.0, h);
            widgets::card(p, card);
            icons::spinner(p, card.left_top() + vec2(36.0, 40.0), 12.0, now, pl.text);
            widgets::fit_text(
                p,
                card.left_top() + vec2(64.0, 30.0),
                Align2::LEFT_CENTER,
                &d.name,
                theme::medium(15.0),
                400.0,
                pl.text,
            );
            widgets::fit_text(
                p,
                card.left_top() + vec2(64.0, 52.0),
                Align2::LEFT_CENTER,
                t(label),
                theme::regular(13.0),
                400.0,
                pl.text_dim,
            );
            if let Some(h) = hint {
                widgets::text_wrapped(
                    p,
                    card.left_top() + vec2(64.0, 70.0),
                    400.0,
                    t(h),
                    theme::regular(13.0),
                    pl.text_dim,
                    Align::LEFT,
                );
            }
            cx.ui.ctx().request_repaint();
            y += h + 28.0;
        }

        widgets::section_label(p, cx.at(col_x + 4.0, y), t(T::MyDevices));
        y += 24.0;
        let list = self.device_list();
        if list.is_empty() {
            let body_h = widgets::text_height(p, t(T::NoDevicesBody), theme::regular(13.0), 440.0);
            let card = cx.rect(col_x, y, 500.0, 110.0 + body_h);
            widgets::card(p, card);
            widgets::fit_text(
                p,
                card.left_top() + vec2(28.0, 34.0),
                Align2::LEFT_CENTER,
                t(T::NoDevicesTitle),
                theme::medium(16.0),
                440.0,
                pl.text,
            );
            widgets::text_wrapped(
                p,
                card.left_top() + vec2(28.0, 54.0),
                440.0,
                t(T::NoDevicesBody),
                theme::regular(13.0),
                pl.text_dim,
                Align::LEFT,
            );
            let b = Rect::from_min_size(
                card.left_top() + vec2(28.0, 66.0 + body_h),
                vec2(
                    widgets::button_width(p, t(T::OpenBtSettings)).min(440.0),
                    38.0,
                ),
            );
            if widgets::button(
                cx.ui,
                p,
                b,
                "open-bt",
                t(T::OpenBtSettings),
                ButtonKind::Primary,
                true,
            )
            .clicked()
            {
                open_bluetooth_settings();
            }
            y += card.height() + 16.0;
        }
        for d in list.iter().take(6) {
            let card = cx.rect(col_x, y, 500.0, 74.0);
            widgets::card(p, card);
            self.device_row(
                cx,
                card,
                d,
                target.as_deref() == Some(d.address.as_str()),
                false,
                false,
            );
            y += 86.0;
        }
        let toggle = if self.show_all {
            t(T::ShowNothingOnly)
        } else {
            t(T::ShowAll)
        };
        if widgets::link(
            cx.ui,
            p,
            cx.at(center, y + 8.0),
            Align2::CENTER_CENTER,
            "show-all",
            toggle,
            pl.text_dim,
        )
        .clicked()
        {
            self.show_all = !self.show_all;
        }

        let bottom = cx.screen.bottom() - cx.o.y;
        let lang_pos = cx.at(center, bottom - 64.0);
        icons::globe(p, lang_pos - vec2(52.0, 0.0), 15.0, pl.text_dim);
        if widgets::link(
            cx.ui,
            p,
            lang_pos - vec2(40.0, 0.0),
            Align2::LEFT_CENTER,
            "lang-link",
            self.current_lang().native_name(),
            pl.text_dim,
        )
        .clicked()
        {
            self.modal = Modal::Language;
        }
        p.text(
            cx.at(center, bottom - 30.0),
            Align2::CENTER_CENTER,
            t(T::Disclaimer),
            theme::regular(11.0),
            pl.text_faint,
        );
        if widgets::link(
            cx.ui,
            p,
            cx.at(W - 32.0, bottom - 30.0),
            Align2::RIGHT_CENTER,
            "logs-link",
            t(T::ViewLogs),
            pl.text_faint,
        )
        .clicked()
        {
            self.modal = Modal::Logs;
        }
    }

    /// One paired device inside `card`: icon, name, status and the action button.
    /// `active` = it's the device this app is using.
    fn device_row(
        &mut self,
        cx: &Cx,
        card: Rect,
        d: &PairedDevice,
        is_target: bool,
        active: bool,
        switching: bool,
    ) -> bool {
        let pl = pal();
        let p = &cx.p;
        let ic = card.left_top() + vec2(38.0, card.height() / 2.0);
        p.circle_filled(ic, 20.0, pl.surface_hi);
        icons::transparency(
            p,
            ic,
            20.0,
            if d.connected { pl.text } else { pl.text_faint },
        );
        if d.connected {
            p.circle_filled(ic + vec2(15.0, 15.0), 4.5, pl.good);
        }
        let (label, kind) = if active {
            (t(T::Disconnect), ButtonKind::Danger)
        } else if is_target {
            (t(T::Cancel), ButtonKind::Secondary)
        } else if switching {
            (t(T::Switch), ButtonKind::Primary)
        } else {
            (t(T::Connect), ButtonKind::Primary)
        };
        let bw = widgets::button_width(p, label).clamp(108.0, 170.0);
        let b = Rect::from_min_size(
            card.right_top() + vec2(-20.0 - bw, card.height() / 2.0 - 18.0),
            vec2(bw, 36.0),
        );
        let text_w = card.width() - 72.0 - bw - 36.0;
        widgets::fit_text(
            p,
            card.left_top() + vec2(72.0, card.height() / 2.0 - 11.0),
            Align2::LEFT_CENTER,
            &d.name,
            theme::medium(15.0),
            text_w,
            pl.text,
        );
        let status = if active {
            t(T::InUse)
        } else if d.connected {
            t(T::ConnectedToMac)
        } else {
            t(T::NotConnected)
        };
        widgets::fit_text(
            p,
            card.left_top() + vec2(72.0, card.height() / 2.0 + 11.0),
            Align2::LEFT_CENTER,
            status,
            theme::regular(12.0),
            text_w,
            pl.text_dim,
        );
        if widgets::button(cx.ui, p, b, ("dev", &d.address), label, kind, true).clicked() {
            if active || is_target {
                self.session.disconnect();
            } else {
                self.connect_to(d);
            }
            return true;
        }
        false
    }
}

fn open_bluetooth_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.BluetoothSettings")
        .spawn();
}

// ---------------------------------------------------------------------------------------
// Dialogs & toasts
// ---------------------------------------------------------------------------------------

impl NothingApp {
    fn modals(&mut self, cx: &Cx, snap: &Snapshot, now: f64) {
        let pl = pal();
        let rebooting = snap.phase == Phase::Rebooting;
        if matches!(self.modal, Modal::None) && !rebooting {
            return;
        }
        let screen = cx.screen;
        cx.p.rect_filled(screen, 0.0, pl.overlay);
        let backdrop = cx
            .ui
            .interact(screen, Id::unique("modal-backdrop"), Sense::click());
        let center = screen.center();

        if rebooting {
            let card = Rect::from_center_size(center, vec2(340.0, 170.0));
            cx.p.rect_filled(card, 22.0, pl.surface);
            icons::spinner(&cx.p, card.center() - vec2(0.0, 22.0), 22.0, now, pl.text);
            cx.p.text(
                card.center() + vec2(0.0, 28.0),
                Align2::CENTER_CENTER,
                t(T::PhaseRebooting),
                theme::medium(15.0),
                pl.text,
            );
            cx.p.text(
                card.center() + vec2(0.0, 52.0),
                Align2::CENTER_CENTER,
                t(T::RestartNote),
                theme::regular(12.5),
                pl.text_dim,
            );
            cx.ui.ctx().request_repaint();
            return;
        }

        let modal = self.modal.clone();
        let (w, h) = self.modal_size(&modal, snap, &cx.p);
        let h = h.min(screen.height() - 40.0);
        let appear = cx.anim_bool(
            (
                "modal-open",
                format!("{:?}", std::mem::discriminant(&modal)),
            ),
            true,
        );
        let card = Rect::from_center_size(center + vec2(0.0, (1.0 - appear) * 16.0), vec2(w, h));
        cx.ui
            .interact(card, Id::unique("modal-card"), Sense::click());
        cx.p.rect_filled(card, 22.0, pl.surface);
        let mc = Cx {
            ui: cx.ui,
            p: cx.p.with_clip_rect(card),
            o: card.min,
            screen: cx.screen,
        };
        let close = match modal {
            Modal::None => false,
            Modal::Gesture { target, slot } => {
                self.modal_gesture(&mc, card, &snap.state, target, slot)
            }
            Modal::Restart(kind) => self.modal_restart(&mc, card, kind),
            Modal::Message { title, body } => modal_message(&mc, card, title, body),
            Modal::Devices => self.modal_devices(&mc, card, snap),
            Modal::Language => self.modal_language(&mc, card),
            Modal::Logs => self.modal_logs(&mc, card, snap, now),
        };
        if close || backdrop.clicked() {
            self.close_modal();
        }
    }

    fn modal_size(&self, modal: &Modal, snap: &Snapshot, p: &Painter) -> (f32, f32) {
        match modal {
            Modal::Gesture { target, slot } => {
                let noise = snap
                    .state
                    .gesture_action(target.device(), slot)
                    .is_some_and(nothing_protocol::gesture::is_noise_control);
                let cycle = if slot.anc_toggle && noise {
                    3.0 * 44.0 + 70.0
                } else {
                    0.0
                };
                (
                    460.0,
                    100.0 + slot.options.len() as f32 * 56.0 + cycle + 80.0,
                )
            }
            Modal::Restart(kind) => (
                420.0,
                150.0 + widgets::text_height(p, &restart_body(*kind), theme::regular(13.5), 356.0),
            ),
            Modal::Message { body, .. } => (
                400.0,
                150.0 + widgets::text_height(p, t(*body), theme::regular(13.5), 336.0),
            ),
            Modal::Devices => {
                let n = self.device_list().len().clamp(1, 6) as f32;
                (540.0, 92.0 + n * 84.0 + 120.0)
            }
            Modal::Language => (380.0, 92.0 + 6.0 * 52.0 + 76.0),
            Modal::Logs => (760.0, 560.0),
            Modal::None => (0.0, 0.0),
        }
    }

    fn modal_devices(&mut self, c: &Cx, card: Rect, snap: &Snapshot) -> bool {
        let pl = pal();
        let p = &c.p;
        p.text(
            c.at(28.0, 38.0),
            Align2::LEFT_CENTER,
            t(T::MyDevices),
            theme::medium(18.0),
            pl.text,
        );
        p.text(
            c.at(28.0, 62.0),
            Align2::LEFT_CENTER,
            t(T::ChangeDevice),
            theme::regular(13.0),
            pl.text_dim,
        );
        let active_addr = snap.device.as_ref().map(|d| d.address.clone());
        let mut y = 88.0;
        let list = self.device_list();
        if list.is_empty() {
            p.text(
                c.at(28.0, y + 20.0),
                Align2::LEFT_CENTER,
                t(T::NoDevicesTitle),
                theme::regular(14.0),
                pl.text_dim,
            );
            y += 84.0;
        }
        let mut close = false;
        for d in list.iter().take(6) {
            let row = c.rect(16.0, y, card.width() - 32.0, 76.0);
            let active = active_addr.as_deref() == Some(d.address.as_str());
            p.rect_filled(
                row,
                16.0,
                if active {
                    pl.surface_hi
                } else {
                    pl.bg.gamma_multiply(if pl.dark { 1.0 } else { 0.0 })
                },
            );
            if !active && !pl.dark {
                p.rect_stroke(
                    row,
                    16.0,
                    Stroke::new(1.0, pl.stroke),
                    egui::StrokeKind::Inside,
                );
            }
            close |= self.device_row(c, row, d, false, active, true);
            y += 84.0;
        }
        let label = if self.show_all {
            t(T::ShowNothingOnly)
        } else {
            t(T::ShowAll)
        };
        if widgets::link(
            c.ui,
            p,
            c.at(28.0, y + 12.0),
            Align2::LEFT_CENTER,
            "dev-show-all",
            label,
            pl.text_dim,
        )
        .clicked()
        {
            self.show_all = !self.show_all;
        }
        let by = card.height() - 64.0;
        let pair_w = p
            .layout_no_wrap(t(T::PairNew).to_string(), theme::medium(14.0), pl.text)
            .size()
            .x
            + 40.0;
        let pair = c.rect(28.0, by, pair_w, 40.0);
        if widgets::button(
            c.ui,
            p,
            pair,
            "dev-pair",
            t(T::PairNew),
            ButtonKind::Secondary,
            true,
        )
        .clicked()
        {
            open_bluetooth_settings();
        }
        let done = c.rect(card.width() - 28.0 - 110.0, by, 110.0, 40.0);
        close
            | widgets::button(
                c.ui,
                p,
                done,
                "dev-done",
                t(T::Done),
                ButtonKind::Primary,
                true,
            )
            .clicked()
    }

    fn modal_restart(&mut self, c: &Cx, card: Rect, kind: RestartKind) -> bool {
        let pl = pal();
        let p = &c.p;
        p.text(
            c.at(32.0, 40.0),
            Align2::LEFT_CENTER,
            t(T::RestartTitle),
            theme::medium(18.0),
            pl.text,
        );
        widgets::text_wrapped(
            p,
            c.at(32.0, 64.0),
            card.width() - 64.0,
            &restart_body(kind),
            theme::regular(13.5),
            pl.text_dim,
            Align::LEFT,
        );
        let y = card.height() - 60.0;
        let ok = c.rect(card.width() - 32.0 - 140.0, y, 140.0, 40.0);
        let cancel = c.rect(card.width() - 32.0 - 140.0 - 12.0 - 120.0, y, 120.0, 40.0);
        if widgets::button(
            c.ui,
            p,
            ok,
            "restart-ok",
            t(T::Restart),
            ButtonKind::Primary,
            true,
        )
        .clicked()
        {
            match kind {
                RestartKind::Codec(codec) => self.act(Action::SetCodec(codec)),
                RestartKind::Dual(on) => self.act(Action::SetDualEnabled(on)),
            }
            return true;
        }
        widgets::button(
            c.ui,
            p,
            cancel,
            "restart-cancel",
            t(T::Cancel),
            ButtonKind::Secondary,
            true,
        )
        .clicked()
    }

    fn modal_language(&mut self, c: &Cx, card: Rect) -> bool {
        let pl = pal();
        let p = &c.p;
        p.text(
            c.at(28.0, 40.0),
            Align2::LEFT_CENTER,
            t(T::Language),
            theme::medium(18.0),
            pl.text,
        );
        let mut y = 72.0;
        let mut options: Vec<(Option<Lang>, String)> = vec![(
            None,
            format!("{} ({})", t(T::System), self.system_lang.native_name()),
        )];
        options.extend(
            Lang::ALL
                .iter()
                .map(|l| (Some(*l), l.native_name().to_string())),
        );
        let mut close = false;
        for (lang, label) in options {
            let r = c.rect(12.0, y, card.width() - 24.0, 48.0);
            if widgets::radio_row(
                c.ui,
                p,
                r,
                ("lang", format!("{lang:?}")),
                &label,
                None,
                self.lang_pref == lang,
                true,
            )
            .clicked()
            {
                self.lang_pref = lang;
                close = true;
            }
            y += 52.0;
        }
        let done = c.rect(
            card.width() - 28.0 - 110.0,
            card.height() - 60.0,
            110.0,
            40.0,
        );
        close
            | widgets::button(
                c.ui,
                p,
                done,
                "lang-done",
                t(T::Done),
                ButtonKind::Primary,
                true,
            )
            .clicked()
    }

    fn modal_logs(&mut self, c: &Cx, card: Rect, snap: &Snapshot, now: f64) -> bool {
        let pl = pal();
        let p = &c.p;
        p.text(
            c.at(28.0, 36.0),
            Align2::LEFT_CENTER,
            t(T::Logs),
            theme::medium(18.0),
            pl.text,
        );
        let box_rect = c.rect(20.0, 60.0, card.width() - 40.0, card.height() - 140.0);
        p.rect_filled(box_rect, 12.0, pl.bg);
        let lp = p.with_clip_rect(box_rect.shrink(8.0));
        let mut y = box_rect.bottom() - 12.0;
        for line in snap.log.iter().rev().take(60) {
            let galley = lp.layout(
                line.clone(),
                egui::FontId::monospace(11.0),
                pl.text_dim,
                box_rect.width() - 24.0,
            );
            y -= galley.size().y + 2.0;
            if y < box_rect.top() {
                break;
            }
            lp.galley(pos2(box_rect.left() + 12.0, y), galley, pl.text_dim);
        }
        let by = card.height() - 60.0;
        let copy = c.rect(card.width() - 28.0 - 120.0 - 12.0 - 120.0, by, 120.0, 40.0);
        if widgets::button(
            c.ui,
            p,
            copy,
            "log-copy",
            t(T::Copy),
            ButtonKind::Secondary,
            true,
        )
        .clicked()
        {
            c.ui.ctx()
                .copy_text(snap.log.iter().cloned().collect::<Vec<_>>().join("\n"));
            self.show_toast(t(T::ToastCopied), now);
        }
        let close = c.rect(card.width() - 28.0 - 120.0, by, 120.0, 40.0);
        widgets::button(
            c.ui,
            p,
            close,
            "log-close",
            t(T::Close),
            ButtonKind::Primary,
            true,
        )
        .clicked()
    }

    fn toast_view(&mut self, cx: &Cx, now: f64) {
        let Some((text, at)) = &self.toast else {
            return;
        };
        let age = (now - at) as f32;
        if age > 2.8 {
            self.toast = None;
            return;
        }
        let alpha = (age / 0.2).min(1.0).min(((2.8 - age) / 0.3).max(0.0));
        let pl = pal();
        let galley =
            cx.p.layout_no_wrap(text.clone(), theme::medium(13.5), pl.on_inverse);
        let size = galley.size() + vec2(40.0, 22.0);
        let x =
            (cx.o.x + SIDEBAR + (W - SIDEBAR) / 2.0).min(cx.screen.right() - size.x / 2.0 - 16.0);
        let rect = Rect::from_center_size(
            pos2(x, cx.screen.bottom() - 44.0 + (1.0 - alpha) * 10.0),
            size,
        );
        let p = cx.faded(alpha);
        p.rect_filled(rect, size.y / 2.0, pl.inverse);
        p.galley(rect.center() - galley.size() / 2.0, galley, pl.on_inverse);
        cx.ui.ctx().request_repaint();
    }
}

pub(super) fn restart_body(kind: RestartKind) -> String {
    match kind {
        RestartKind::Codec(Codec::Ldac) => format!("{} {}", t(T::LdacWarning), t(T::RestartBody)),
        _ => t(T::RestartBody).to_string(),
    }
}

fn modal_message(c: &Cx, card: Rect, title: T, body: T) -> bool {
    let pl = pal();
    let p = &c.p;
    p.text(
        c.at(32.0, 40.0),
        Align2::LEFT_CENTER,
        t(title),
        theme::medium(18.0),
        pl.text,
    );
    widgets::text_wrapped(
        p,
        c.at(32.0, 64.0),
        card.width() - 64.0,
        t(body),
        theme::regular(13.5),
        pl.text_dim,
        Align::LEFT,
    );
    let ok = c.rect(
        card.width() - 32.0 - 110.0,
        card.height() - 60.0,
        110.0,
        40.0,
    );
    widgets::button(c.ui, p, ok, "msg-ok", t(T::Done), ButtonKind::Primary, true).clicked()
}
