//! Vector icons drawn with the painter, in a dotted, monochrome style.

use std::f32::consts::{PI, TAU};

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2};

fn arc(p: &Painter, c: Pos2, r: f32, from: f32, to: f32, stroke: Stroke) {
    let n = 18;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = from + (to - from) * i as f32 / n as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect();
    p.add(Shape::line(pts, stroke));
}

fn slash(p: &Painter, c: Pos2, half: f32, stroke: Stroke) {
    let d = vec2(half, half) * std::f32::consts::FRAC_1_SQRT_2;
    p.line_segment([c - d, c + d], stroke);
}

/// Noise cancellation: solid ring around a dot.
pub fn noise_cancelling(p: &Painter, c: Pos2, s: f32, color: Color32) {
    p.circle_stroke(c, s * 0.45, Stroke::new(s * 0.1, color));
    p.circle_filled(c, s * 0.2, color);
}

/// Transparency: a ring of dots around a dot.
pub fn transparency(p: &Painter, c: Pos2, s: f32, color: Color32) {
    for i in 0..8 {
        let a = i as f32 / 8.0 * TAU - TAU / 4.0;
        p.circle_filled(c + vec2(a.cos(), a.sin()) * s * 0.46, s * 0.065, color);
    }
    p.circle_filled(c, s * 0.13, color);
}

/// Off: ring with a slash.
pub fn off(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let stroke = Stroke::new(s * 0.085, color);
    p.circle_stroke(c, s * 0.45, stroke);
    slash(p, c, s * 0.55, stroke);
}

/// Speaker body (Ultra Bass).
pub fn speaker(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let stroke = Stroke::new(s * 0.08, color);
    let body = Rect::from_center_size(c, vec2(s * 0.72, s * 0.92));
    p.rect_stroke(body, s * 0.18, stroke, StrokeKind::Middle);
    p.circle_filled(c + vec2(0.0, -s * 0.2), s * 0.08, color);
    p.circle_stroke(c + vec2(0.0, s * 0.14), s * 0.16, stroke);
}

pub fn overview(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let stroke = Stroke::new(s * 0.08, color);
    let q = s * 0.36;
    for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let r = Rect::from_center_size(c + vec2(dx, dy) * (q / 2.0 + s * 0.04), vec2(q, q));
        p.rect_stroke(r, s * 0.08, stroke, StrokeKind::Middle);
    }
}

/// Three EQ bars made of dots.
pub fn sound(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let heights = [3, 5, 2];
    let pitch = s * 0.19;
    for (i, h) in heights.iter().enumerate() {
        let x = c.x + (i as f32 - 1.0) * s * 0.32;
        for j in 0..5 {
            let y = c.y + s * 0.38 - j as f32 * pitch;
            let col = if j < *h {
                color
            } else {
                color.gamma_multiply(0.25)
            };
            p.circle_filled(pos2(x, y), s * 0.07, col);
        }
    }
}

/// A fingertip with ripples (gestures).
pub fn controls(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let stroke = Stroke::new(s * 0.08, color);
    p.circle_filled(c, s * 0.14, color);
    arc(p, c, s * 0.3, -PI * 0.85, -PI * 0.15, stroke);
    arc(
        p,
        c,
        s * 0.46,
        -PI * 0.8,
        -PI * 0.2,
        Stroke::new(s * 0.08, color.gamma_multiply(0.6)),
    );
    arc(p, c, s * 0.3, PI * 0.15, PI * 0.85, stroke);
}

/// Sound waves coming out of a dot (find my earbuds).
pub fn find(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let stroke = Stroke::new(s * 0.08, color);
    let o = c - vec2(s * 0.22, 0.0);
    p.circle_filled(o, s * 0.12, color);
    arc(p, o, s * 0.3, -PI / 4.0, PI / 4.0, stroke);
    arc(
        p,
        o,
        s * 0.52,
        -PI / 4.0,
        PI / 4.0,
        Stroke::new(s * 0.08, color.gamma_multiply(0.6)),
    );
}

/// Ear tip with a check (fit test).
pub fn fit(p: &Painter, c: Pos2, s: f32, color: Color32) {
    p.circle_stroke(c, s * 0.42, Stroke::new(s * 0.08, color));
    checkmark(p, c, s * 0.5, color);
}

/// Gear made of a ring and dots.
pub fn settings(p: &Painter, c: Pos2, s: f32, color: Color32) {
    p.circle_stroke(c, s * 0.22, Stroke::new(s * 0.08, color));
    for i in 0..8 {
        let a = i as f32 / 8.0 * TAU;
        p.circle_filled(c + vec2(a.cos(), a.sin()) * s * 0.42, s * 0.075, color);
    }
}

pub fn globe(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let stroke = Stroke::new(s * 0.07, color);
    p.circle_stroke(c, s * 0.42, stroke);
    p.line_segment([c - vec2(s * 0.42, 0.0), c + vec2(s * 0.42, 0.0)], stroke);
    let pts: Vec<Pos2> = (0..=16)
        .map(|i| {
            let a = -PI / 2.0 + PI * i as f32 / 16.0;
            c + vec2(a.cos() * s * 0.18, a.sin() * s * 0.42)
        })
        .collect();
    let mirrored: Vec<Pos2> = pts.iter().map(|q| pos2(2.0 * c.x - q.x, q.y)).collect();
    p.add(Shape::line(pts, stroke));
    p.add(Shape::line(mirrored, stroke));
}

pub fn warning(p: &Painter, c: Pos2, s: f32, color: Color32) {
    p.circle_filled(c, s * 0.46, color);
    let ink = Color32::BLACK;
    p.line_segment(
        [c + vec2(0.0, -s * 0.22), c + vec2(0.0, s * 0.06)],
        Stroke::new(s * 0.1, ink),
    );
    p.circle_filled(c + vec2(0.0, s * 0.22), s * 0.06, ink);
}

/// Small lightning bolt (charging).
pub fn bolt(p: &Painter, c: Pos2, s: f32, color: Color32) {
    let top = [
        c + vec2(s * 0.12, -s * 0.5),
        c + vec2(-s * 0.28, s * 0.08),
        c + vec2(s * 0.02, s * 0.08),
    ];
    let bottom = [
        c + vec2(-s * 0.12, s * 0.5),
        c + vec2(s * 0.28, -s * 0.08),
        c + vec2(-s * 0.02, -s * 0.08),
    ];
    p.add(Shape::convex_polygon(top.to_vec(), color, Stroke::NONE));
    p.add(Shape::convex_polygon(bottom.to_vec(), color, Stroke::NONE));
}

pub fn chevron(p: &Painter, c: Pos2, size: f32, left: bool, color: Color32) {
    let s = if left { 1.0 } else { -1.0 };
    let stroke = Stroke::new(1.6, color);
    p.line_segment(
        [
            c + vec2(s * size * 0.25, -size * 0.5),
            c + vec2(-s * size * 0.25, 0.0),
        ],
        stroke,
    );
    p.line_segment(
        [
            c + vec2(-s * size * 0.25, 0.0),
            c + vec2(s * size * 0.25, size * 0.5),
        ],
        stroke,
    );
}

pub fn chevron_down(p: &Painter, c: Pos2, size: f32, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    p.line_segment(
        [
            c + vec2(-size * 0.5, -size * 0.25),
            c + vec2(0.0, size * 0.25),
        ],
        stroke,
    );
    p.line_segment(
        [
            c + vec2(0.0, size * 0.25),
            c + vec2(size * 0.5, -size * 0.25),
        ],
        stroke,
    );
}

pub fn checkmark(p: &Painter, c: Pos2, size: f32, color: Color32) {
    let stroke = Stroke::new((size * 0.14).max(1.5), color);
    let a = c + vec2(-size * 0.45, 0.0);
    let b = c + vec2(-size * 0.1, size * 0.35);
    let d = c + vec2(size * 0.5, -size * 0.4);
    p.line_segment([a, b], stroke);
    p.line_segment([b, d], stroke);
}

/// Rotating dot ring, used while loading.
pub fn spinner(p: &Painter, c: Pos2, r: f32, time: f64, color: Color32) {
    let n = 10;
    let head = ((time * 1.4).fract() * n as f64) as i32;
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU - TAU / 4.0;
        let age = (head - i).rem_euclid(n) as f32 / n as f32;
        let alpha = (1.0 - age).powf(1.6).max(0.12);
        p.circle_filled(
            c + vec2(a.cos(), a.sin()) * r,
            r * 0.11,
            color.gamma_multiply(alpha),
        );
    }
}
