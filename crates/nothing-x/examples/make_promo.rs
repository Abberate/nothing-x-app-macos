//! Social media images: dot-matrix headline + app screenshots.
//!
//! cargo run --release -p nothing-x --example make_promo -- <out_dir> <main.png> <small1.png> <small2.png>
//! Writes `promo-carre.png` (2160×2160) and `promo-portrait.png` (2160×2700, 4:5 feed format).

use image::{Rgba, RgbaImage, imageops};
use nothing_x::ui::dotmatrix::glyph;

const BG: [u8; 4] = [0, 0, 0, 255];
const WHITE: [u8; 4] = [245, 245, 245, 255];
const GREY: [u8; 4] = [150, 150, 155, 255];
const FAINT: [u8; 4] = [95, 95, 100, 255];
const RED: [u8; 4] = [0xD7, 0x19, 0x21, 255];

fn blend(img: &mut RgbaImage, x: i64, y: i64, c: [u8; 4], a: f32) {
    if x < 0 || y < 0 || x >= img.width() as i64 || y >= img.height() as i64 || a <= 0.0 {
        return;
    }
    let p = img.get_pixel_mut(x as u32, y as u32);
    for i in 0..3 {
        p[i] = (p[i] as f32 * (1.0 - a) + c[i] as f32 * a).round() as u8;
    }
    p[3] = 255;
}

/// Anti-aliased filled circle (4×4 supersampling).
fn circle(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, c: [u8; 4]) {
    let (x0, x1) = ((cx - r - 1.0).floor() as i64, (cx + r + 1.0).ceil() as i64);
    let (y0, y1) = ((cy - r - 1.0).floor() as i64, (cy + r + 1.0).ceil() as i64);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let mut hits = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let px = x as f32 + (sx as f32 + 0.5) / 4.0;
                    let py = y as f32 + (sy as f32 + 0.5) / 4.0;
                    if (px - cx).powi(2) + (py - cy).powi(2) <= r * r {
                        hits += 1;
                    }
                }
            }
            blend(img, x, y, c, hits as f32 / 16.0);
        }
    }
}

fn advance(ch: char) -> f32 {
    if ch == ' ' { 3.0 } else { 6.0 }
}

fn dot_width(text: &str, height: f32) -> f32 {
    (text.chars().map(advance).sum::<f32>() - 1.0) * height / 7.0
}

/// Dot-matrix text centred on `cx`, top at `top`. Returns the right edge.
fn dot_text(
    img: &mut RgbaImage,
    text: &str,
    cx: f32,
    top: f32,
    height: f32,
    c: [u8; 4],
    bold: bool,
) -> f32 {
    let pitch = height / 7.0;
    let mut x = cx - dot_width(text, height) / 2.0;
    let r = pitch * if bold { 0.5 } else { 0.36 };
    for ch in text.chars() {
        if let Some(rows) = glyph(ch) {
            for (ry, row) in rows.iter().enumerate() {
                for (rx, cell) in row.bytes().enumerate() {
                    if cell == b'#' {
                        circle(
                            img,
                            x + rx as f32 * pitch + pitch / 2.0,
                            top + ry as f32 * pitch + pitch / 2.0,
                            r,
                            c,
                        );
                    }
                }
            }
        }
        x += advance(ch) * pitch;
    }
    x - pitch
}

/// Pastes `shot` scaled to `width`, with rounded corners and a thin border. Returns its height.
fn paste(canvas: &mut RgbaImage, shot: &RgbaImage, x: u32, y: u32, width: u32, radius: f32) -> u32 {
    let height = (shot.height() as f32 * width as f32 / shot.width() as f32).round() as u32;
    let scaled = imageops::resize(shot, width, height, imageops::FilterType::Lanczos3);
    let (w, h) = (width as f32, height as f32);
    let inside = |px: f32, py: f32, rad: f32| {
        let cx = px.clamp(rad, w - rad);
        let cy = py.clamp(rad, h - rad);
        (px - cx).powi(2) + (py - cy).powi(2) <= rad * rad
    };
    for py in 0..height {
        for px in 0..width {
            let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
            let near_corner = (fx < radius || fx > w - radius) && (fy < radius || fy > h - radius);
            let coverage = if near_corner {
                let mut hits = 0;
                for s in 0..16 {
                    let sx = px as f32 + ((s % 4) as f32 + 0.5) / 4.0;
                    let sy = py as f32 + ((s / 4) as f32 + 0.5) / 4.0;
                    if inside(sx, sy, radius) {
                        hits += 1;
                    }
                }
                hits as f32 / 16.0
            } else {
                1.0
            };
            let src = scaled.get_pixel(px, py);
            // Thin border so dark screenshots stand out on the black background.
            let edge = !inside(fx, fy, radius)
                || fx < 2.0
                || fy < 2.0
                || fx > w - 2.0
                || fy > h - 2.0
                || !inside(fx.clamp(2.0, w - 2.0), fy.clamp(2.0, h - 2.0), radius - 2.0);
            let color = if edge {
                [52, 52, 56, 255]
            } else {
                [src[0], src[1], src[2], 255]
            };
            blend(canvas, (x + px) as i64, (y + py) as i64, color, coverage);
        }
    }
    height
}

fn headline(img: &mut RgbaImage, top: f32) -> f32 {
    let cx = img.width() as f32 / 2.0;
    let right = dot_text(img, "NOTHING X APP", cx - 30.0, top, 112.0, WHITE, true);
    circle(img, right + 52.0, top + 14.0, 14.0, RED);
    dot_text(
        img,
        "VOS ECOUTEURS CMF, SUR MAC",
        cx,
        top + 172.0,
        42.0,
        GREY,
        false,
    );
    top + 172.0 + 42.0
}

fn footer(img: &mut RgbaImage, top: f32) {
    let cx = img.width() as f32 / 2.0;
    dot_text(
        img,
        "5 LANGUES - SOMBRE & CLAIR - MACOS",
        cx,
        top,
        30.0,
        FAINT,
        false,
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        eprintln!("usage: make_promo <out_dir> <main.png> <small1.png> <small2.png>");
        std::process::exit(2);
    }
    let out = std::path::PathBuf::from(&args[0]);
    let load = |p: &str| image::open(p).expect("screenshot").to_rgba8();
    let (main, s1, s2) = (load(&args[1]), load(&args[2]), load(&args[3]));

    // Square.
    let mut sq = RgbaImage::from_pixel(2160, 2160, Rgba(BG));
    let y = headline(&mut sq, 150.0) + 110.0;
    let h = paste(&mut sq, &main, 120, y as u32, 1920, 40.0);
    footer(&mut sq, y + h as f32 + 90.0);
    sq.save(out.join("promo-carre.png")).expect("write square");

    // Portrait 4:5.
    let mut pt = RgbaImage::from_pixel(2160, 2700, Rgba(BG));
    let y = headline(&mut pt, 140.0) + 90.0;
    let h = paste(&mut pt, &main, 180, y as u32, 1800, 40.0);
    let y2 = y as u32 + h + 40;
    let h2 = paste(&mut pt, &s1, 180, y2, 880, 28.0);
    paste(&mut pt, &s2, 1100, y2, 880, 28.0);
    footer(&mut pt, (y2 + h2) as f32 + 70.0);
    pt.save(out.join("promo-portrait.png"))
        .expect("write portrait");
    println!(
        "wrote promo-carre.png and promo-portrait.png in {}",
        out.display()
    );
}
