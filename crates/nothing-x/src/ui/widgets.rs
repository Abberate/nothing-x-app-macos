//! Themed building blocks (cards, buttons, segmented controls, toggles…).

use std::cell::Cell;

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align, Align2, Color32, CursorIcon, FontId, Id, Painter, Pos2, Rect, Response, Sense, Stroke,
    StrokeKind, Ui, pos2, vec2,
};

use super::icons;
use super::theme::{self, pal};

pub type IconFn = fn(&Painter, Pos2, f32, Color32);

thread_local! {
    static INPUT_BLOCKED: Cell<bool> = const { Cell::new(false) };
    static CLIP: Cell<Option<Rect>> = const { Cell::new(None) };
}

/// While set (e.g. under a dialog), [`hit`] targets never hover nor click.
pub fn set_input_blocked(blocked: bool) {
    INPUT_BLOCKED.with(|b| b.set(blocked));
}

pub fn input_blocked() -> bool {
    INPUT_BLOCKED.with(|b| b.get())
}

/// Restricts interaction to a region (scrolling content).
pub fn set_clip(clip: Option<Rect>) {
    CLIP.with(|c| c.set(clip));
}

/// Interaction area for `rect`, honouring the input block and the clip region.
pub fn hit_sense(
    ui: &Ui,
    rect: Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
    sense: Sense,
) -> Response {
    let mut rect = rect;
    if let Some(clip) = CLIP.with(|c| c.get()) {
        rect = rect.intersect(clip);
    }
    if input_blocked() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return ui.interact(Rect::NOTHING, Id::unique(id), Sense::hover());
    }
    ui.interact(rect, Id::unique(id), sense)
        .on_hover_cursor(CursorIcon::PointingHand)
}

pub fn hit(ui: &Ui, rect: Rect, id: impl std::hash::Hash + std::fmt::Debug) -> Response {
    hit_sense(ui, rect, id, Sense::click())
}

pub fn card(p: &Painter, rect: Rect) {
    p.rect_filled(rect, 18.0, pal().surface);
}

/// Small uppercase, letter-spaced section label.
pub fn section_label(p: &Painter, pos: Pos2, text: &str) -> Rect {
    let mut job = LayoutJob::default();
    job.append(
        &text.to_uppercase(),
        0.0,
        TextFormat {
            font_id: theme::medium(11.0),
            color: pal().text_faint,
            extra_letter_spacing: 1.2,
            ..Default::default()
        },
    );
    let galley = p.layout_job(job);
    let rect = Rect::from_min_size(pos, galley.size());
    p.galley(pos, galley, pal().text_faint);
    rect
}

/// Wrapped text; `align` is the horizontal alignment around `pos.x`. Returns its height.
pub fn text_wrapped(
    p: &Painter,
    pos: Pos2,
    width: f32,
    s: &str,
    font: FontId,
    color: Color32,
    align: Align,
) -> f32 {
    let mut job = LayoutJob::simple(s.to_string(), font, color, width);
    job.halign = align;
    let galley = p.layout_job(job);
    let h = galley.size().y;
    p.galley(pos, galley, color);
    h
}

pub fn text_height(p: &Painter, s: &str, font: FontId, width: f32) -> f32 {
    p.layout(s.to_string(), font, Color32::WHITE, width)
        .size()
        .y
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Secondary,
    Danger,
    /// Filled red (destructive / active alarm).
    Accent,
}

pub fn button(
    ui: &Ui,
    p: &Painter,
    rect: Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: &str,
    kind: ButtonKind,
    enabled: bool,
) -> Response {
    let pl = pal();
    let resp = if enabled {
        hit(ui, rect, id)
    } else {
        ui.interact(Rect::NOTHING, Id::unique(id), Sense::hover())
    };
    let hover = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
    let (bg, fg) = match kind {
        ButtonKind::Primary => (pl.inverse, pl.on_inverse),
        ButtonKind::Secondary => (pl.surface_hi, pl.text),
        ButtonKind::Danger => (pl.accent.gamma_multiply(0.16), pl.accent),
        ButtonKind::Accent => (pl.accent, Color32::WHITE),
    };
    let bg = lerp_color(
        bg,
        if kind == ButtonKind::Primary {
            pl.inverse.gamma_multiply(0.85)
        } else {
            pl.stroke
        },
        hover,
    );
    let alpha = if enabled { 1.0 } else { 0.4 };
    p.rect_filled(rect, rect.height() / 2.0, bg.gamma_multiply(alpha));
    fit_text(
        p,
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        theme::medium(14.0),
        rect.width() - 22.0,
        fg.gamma_multiply(alpha),
    );
    resp
}

/// Width a button needs for `label` (padding included).
pub fn button_width(p: &Painter, label: &str) -> f32 {
    text_width(p, label, &theme::medium(14.0)) + 40.0
}

/// Text-only link.
pub fn link(
    ui: &Ui,
    p: &Painter,
    pos: Pos2,
    anchor: Align2,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: &str,
    color: Color32,
) -> Response {
    let galley = p.layout_no_wrap(label.to_string(), theme::medium(13.0), color);
    let rect = anchor.anchor_size(pos, galley.size());
    let resp = hit(ui, rect.expand(4.0), id);
    let c = if resp.hovered() { pal().text } else { color };
    p.galley(
        rect.min,
        p.layout_no_wrap(label.to_string(), theme::medium(13.0), c),
        c,
    );
    resp
}

pub struct SegItem<'a> {
    pub label: &'a str,
    pub icon: Option<IconFn>,
}

/// Segmented control with a sliding pill. Returns the index clicked (if different).
pub fn segmented(
    ui: &Ui,
    p: &Painter,
    rect: Rect,
    id: &str,
    items: &[SegItem],
    selected: Option<usize>,
) -> Option<usize> {
    let pl = pal();
    let n = items.len().max(1) as f32;
    let pad = 4.0;
    let w = (rect.width() - pad * 2.0) / n;
    let radius = (rect.height() - pad * 2.0) / 2.0;
    let radius = radius.min(22.0);
    let track_radius = radius + pad;
    p.rect_filled(rect, track_radius, pl.surface_hi);
    if let Some(sel) = selected {
        let x = ui
            .ctx()
            .animate_value_with_time(Id::unique((id, "pill")), sel as f32 * w, 0.22);
        let pill = Rect::from_min_size(
            rect.min + vec2(pad + x, pad),
            vec2(w, rect.height() - pad * 2.0),
        );
        p.rect_filled(pill, radius, pl.inverse);
    }
    let mut clicked = None;
    for (i, item) in items.iter().enumerate() {
        let r = Rect::from_min_size(
            rect.min + vec2(pad + i as f32 * w, pad),
            vec2(w, rect.height() - pad * 2.0),
        );
        let resp = hit(ui, r, (id, i));
        let on = selected == Some(i);
        let t = ui
            .ctx()
            .animate_bool_with_time(Id::unique((id, i, "on")), on, 0.18);
        let fg = lerp_color(
            if resp.hovered() { pl.text } else { pl.text_dim },
            pl.on_inverse,
            t,
        );
        match item.icon {
            Some(icon) if r.height() >= 52.0 => {
                icon(p, r.center() - vec2(0.0, 11.0), 22.0, fg);
                fit_text(
                    p,
                    r.center() + vec2(0.0, 17.0),
                    Align2::CENTER_CENTER,
                    item.label,
                    theme::medium(12.5),
                    r.width() - 14.0,
                    fg,
                );
            }
            Some(icon) => {
                let (font, label) = fitted(p, item.label, theme::medium(13.0), r.width() - 44.0);
                let galley = p.layout_no_wrap(label, font, fg);
                let total = 18.0 + 8.0 + galley.size().x;
                let x0 = r.center().x - total / 2.0;
                icon(p, pos2(x0 + 9.0, r.center().y), 18.0, fg);
                p.galley(
                    pos2(x0 + 26.0, r.center().y - galley.size().y / 2.0),
                    galley,
                    fg,
                );
            }
            None => {
                fit_text(
                    p,
                    r.center(),
                    Align2::CENTER_CENTER,
                    item.label,
                    theme::medium(13.0),
                    r.width() - 14.0,
                    fg,
                );
            }
        }
        if resp.clicked() && !on {
            clicked = Some(i);
        }
    }
    clicked
}

fn text_width(p: &Painter, text: &str, font: &FontId) -> f32 {
    p.layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE)
        .size()
        .x
}

/// Shrinks `font` (down to 80 %) and, if still needed, truncates `text` with "…" so it
/// fits on one line in `max_w`. Overflows are reported in audit mode.
fn fitted(p: &Painter, text: &str, font: FontId, max_w: f32) -> (FontId, String) {
    let w = text_width(p, text, &font);
    if w <= max_w || max_w <= 0.0 {
        return (font, text.to_string());
    }
    let base = font.size;
    let mut size = (base * max_w / w).max(base * 0.8);
    // Glyph widths don't scale exactly linearly (hinting): step down until it fits.
    while size > base * 0.8 && text_width(p, text, &FontId::new(size, font.family.clone())) > max_w
    {
        size -= 0.25;
    }
    let font = FontId::new(size.max(base * 0.8), font.family);
    if size < base * 0.86 || text_width(p, text, &font) > max_w {
        audit("line", text, w, max_w);
    }
    if text_width(p, text, &font) <= max_w {
        return (font, text.to_string());
    }
    let mut chars: Vec<char> = text.chars().collect();
    while chars.len() > 1 {
        chars.pop();
        let candidate = format!("{}…", chars.iter().collect::<String>().trim_end());
        if text_width(p, &candidate, &font) <= max_w {
            return (font, candidate);
        }
    }
    (font, "…".into())
}

/// One line of text that never overflows `max_w` (see [`fitted`]).
pub fn fit_text(
    p: &Painter,
    pos: Pos2,
    anchor: Align2,
    text: &str,
    font: FontId,
    max_w: f32,
    color: Color32,
) -> Rect {
    let (font, text) = fitted(p, text, font, max_w);
    p.text(pos, anchor, text, font, color)
}

thread_local! {
    static AUDITED: std::cell::RefCell<std::collections::HashSet<String>> = std::cell::RefCell::new(Default::default());
}

/// Reports text that doesn't fit its area (enabled with `NOTHINGX_CHECK_TEXT=1`).
pub fn audit(kind: &str, text: &str, need: f32, have: f32) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if !*ENABLED.get_or_init(|| std::env::var_os("NOTHINGX_CHECK_TEXT").is_some()) {
        return;
    }
    let key = format!("{kind}|{text}");
    if AUDITED.with(|a| a.borrow_mut().insert(key)) {
        eprintln!(
            "TEXT-FIT [{}] {kind}: \"{text}\" needs {need:.0}, has {have:.0}",
            super::i18n::lang().code()
        );
    }
}

/// Wrapped text that must fit in `max_h`; reports it in audit mode otherwise.
#[allow(clippy::too_many_arguments)]
pub fn text_box(
    p: &Painter,
    pos: Pos2,
    width: f32,
    max_h: f32,
    s: &str,
    font: FontId,
    color: Color32,
    align: Align,
) -> f32 {
    let h = text_wrapped(p, pos, width, s, font, color, align);
    if h > max_h + 0.5 {
        audit("box", s, h, max_h);
    }
    h
}

/// iOS-style switch, 46×28, right edge at `right_center`. Monochrome: inverse track when on.
pub fn toggle(
    ui: &Ui,
    p: &Painter,
    right_center: Pos2,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    on: bool,
    enabled: bool,
) -> Response {
    let pl = pal();
    let rect = Rect::from_min_size(right_center - vec2(46.0, 14.0), vec2(46.0, 28.0));
    let resp = if enabled {
        hit(ui, rect.expand(4.0), id)
    } else {
        ui.interact(Rect::NOTHING, Id::unique(id), Sense::hover())
    };
    let t = ui
        .ctx()
        .animate_bool_with_time(Id::unique(("toggle", id)), on, 0.15);
    let alpha = if enabled { 1.0 } else { 0.45 };
    let track = lerp_color(pl.stroke, pl.inverse, t).gamma_multiply(alpha);
    p.rect_filled(rect, 14.0, track);
    let knob = lerp_color(pl.text_dim, pl.on_inverse, t).gamma_multiply(alpha);
    p.circle_filled(
        pos2(rect.left() + 14.0 + 18.0 * t, rect.center().y),
        11.0,
        knob,
    );
    resp
}

pub fn radio(p: &Painter, c: Pos2, selected: bool) {
    let pl = pal();
    if selected {
        p.circle_filled(c, 11.0, pl.inverse);
        p.circle_filled(c, 4.0, pl.on_inverse);
    } else {
        p.circle_stroke(c, 10.0, Stroke::new(1.5, pl.text_faint));
    }
}

/// Selectable row with a title, optional description and a radio on the right.
#[allow(clippy::too_many_arguments)]
pub fn radio_row(
    ui: &Ui,
    p: &Painter,
    rect: Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
    title: &str,
    desc: Option<&str>,
    selected: bool,
    enabled: bool,
) -> Response {
    let pl = pal();
    let resp = if enabled {
        hit(ui, rect, id)
    } else {
        ui.interact(Rect::NOTHING, Id::unique(id), Sense::hover())
    };
    let hover = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
    p.rect_filled(rect, 14.0, pl.surface_hi.gamma_multiply(hover * 0.8));
    let alpha = if enabled { 1.0 } else { 0.45 };
    let text_w = rect.width() - 76.0;
    if let Some(d) = desc {
        let title_h = text_height(p, title, theme::medium(14.5), text_w);
        let desc_h = text_height(p, d, theme::regular(12.5), text_w);
        let top = rect.center().y - (title_h + 3.0 + desc_h) / 2.0;
        text_wrapped(
            p,
            pos2(rect.left() + 18.0, top),
            text_w,
            title,
            theme::medium(14.5),
            pl.text.gamma_multiply(alpha),
            Align::LEFT,
        );
        text_wrapped(
            p,
            pos2(rect.left() + 18.0, top + title_h + 3.0),
            text_w,
            d,
            theme::regular(12.5),
            pl.text_dim.gamma_multiply(alpha),
            Align::LEFT,
        );
        let used = title_h + 3.0 + desc_h;
        if used > rect.height() - 8.0 {
            audit("row", d, used, rect.height() - 8.0);
        }
    } else {
        fit_text(
            p,
            pos2(rect.left() + 18.0, rect.center().y),
            Align2::LEFT_CENTER,
            title,
            theme::medium(14.5),
            text_w,
            pl.text.gamma_multiply(alpha),
        );
    }
    radio(p, pos2(rect.right() - 30.0, rect.center().y), selected);
    resp
}

pub fn checkbox(p: &Painter, c: Pos2, checked: bool) {
    let pl = pal();
    let r = Rect::from_center_size(c, vec2(20.0, 20.0));
    if checked {
        p.rect_filled(r, 6.0, pl.inverse);
        icons::checkmark(p, c, 11.0, pl.on_inverse);
    } else {
        p.rect_stroke(r, 6.0, Stroke::new(1.5, pl.text_faint), StrokeKind::Inside);
    }
}

/// Sidebar navigation entry.
pub fn nav_item(
    ui: &Ui,
    p: &Painter,
    rect: Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
    icon: IconFn,
    label: &str,
    selected: bool,
) -> Response {
    let pl = pal();
    let resp = hit(ui, rect, id);
    let hover = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.12);
    let sel = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("s"), selected, 0.18);
    let bg = lerp_color(pl.surface.gamma_multiply(hover * 0.7), pl.surface_hi, sel);
    p.rect_filled(rect, 12.0, bg);
    let fg = lerp_color(pl.text_dim, pl.text, sel.max(hover * 0.6));
    icon(p, pos2(rect.left() + 22.0, rect.center().y), 18.0, fg);
    fit_text(
        p,
        pos2(rect.left() + 44.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        theme::medium(14.0),
        rect.width() - 44.0 - 26.0,
        fg,
    );
    if selected {
        p.circle_filled(pos2(rect.right() - 14.0, rect.center().y), 3.0, pl.accent);
    }
    resp
}

/// Thin battery bar; red when low.
pub fn battery_bar(p: &Painter, rect: Rect, percent: Option<u8>) {
    let pl = pal();
    p.rect_filled(rect, rect.height() / 2.0, pl.stroke);
    if let Some(pct) = percent {
        let w = rect.width() * f32::from(pct.min(100)) / 100.0;
        if w > 0.5 {
            let color = if pct <= 15 { pl.accent } else { pl.text };
            p.rect_filled(
                Rect::from_min_size(rect.min, vec2(w.max(rect.height()), rect.height())),
                rect.height() / 2.0,
                color,
            );
        }
    }
}

/// Height a settings row needs for its texts (at least `min`).
pub fn row_height(
    p: &Painter,
    width: f32,
    title: &str,
    desc: Option<&str>,
    right_reserve: f32,
    min: f32,
) -> f32 {
    let w = width - right_reserve - 20.0;
    let th = text_height(p, title, theme::medium(14.5), w);
    let dh = desc
        .map(|d| 3.0 + text_height(p, d, theme::regular(12.5), w))
        .unwrap_or(0.0);
    (th + dh + 34.0).max(min)
}

/// Settings-style row: title + optional description on the left; caller draws the
/// control on the right.
pub fn row_text(p: &Painter, rect: Rect, title: &str, desc: Option<&str>, right_reserve: f32) {
    let pl = pal();
    let w = rect.width() - right_reserve - 20.0;
    match desc {
        Some(d) => {
            let th = text_height(p, title, theme::medium(14.5), w);
            let dh = text_height(p, d, theme::regular(12.5), w);
            let top = rect.center().y - (th + 3.0 + dh) / 2.0;
            text_wrapped(
                p,
                pos2(rect.left() + 20.0, top),
                w,
                title,
                theme::medium(14.5),
                pl.text,
                Align::LEFT,
            );
            text_wrapped(
                p,
                pos2(rect.left() + 20.0, top + th + 3.0),
                w,
                d,
                theme::regular(12.5),
                pl.text_dim,
                Align::LEFT,
            );
        }
        None => {
            fit_text(
                p,
                pos2(rect.left() + 20.0, rect.center().y),
                Align2::LEFT_CENTER,
                title,
                theme::medium(14.5),
                w,
                pl.text,
            );
        }
    }
}

pub fn divider(p: &Painter, x0: f32, x1: f32, y: f32) {
    p.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.0, pal().stroke));
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}
