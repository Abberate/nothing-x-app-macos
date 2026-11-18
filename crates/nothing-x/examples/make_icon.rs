//! Renders the app icon (1024×1024 PNG): a dark squircle with a dot-matrix "X".
//! cargo run -p nothing-x --example make_icon -- <out.png>

use image::{Rgba, RgbaImage};

const SIZE: u32 = 1024;
const SS: u32 = 4; // supersampling per axis

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "icon_1024.png".into());
    let mut img = RgbaImage::new(SIZE, SIZE);

    // macOS icon grid: 824 px body, ~185 px corner radius.
    let body = 824.0f32;
    let origin = (SIZE as f32 - body) / 2.0;
    let radius = 185.0f32;

    let x_pattern = [
        "#.....#", ".#...#.", "..#.#..", "...#...", "..#.#..", ".#...#.", "#.....#",
    ];
    let pitch = 74.0f32;
    let grid_w = pitch * 7.0;
    let gx = SIZE as f32 / 2.0 - grid_w / 2.0;
    let gy = SIZE as f32 / 2.0 - grid_w / 2.0 + 10.0;
    let mut dots: Vec<(f32, f32, f32, [u8; 4])> = Vec::new();
    for (r, row) in x_pattern.iter().enumerate() {
        for (c, cell) in row.bytes().enumerate() {
            let cx = gx + c as f32 * pitch + pitch / 2.0;
            let cy = gy + r as f32 * pitch + pitch / 2.0;
            if cell == b'#' {
                dots.push((cx, cy, pitch * 0.40, [255, 255, 255, 255]));
            } else {
                dots.push((cx, cy, pitch * 0.11, [255, 255, 255, 40]));
            }
        }
    }
    // Red accent dot, top right.
    dots.push((
        origin + body - 105.0,
        origin + 105.0,
        26.0,
        [0xD7, 0x1E, 0x28, 255],
    ));

    for y in 0..SIZE {
        for x in 0..SIZE {
            let mut acc = [0f32; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    let px = x as f32 + (sx as f32 + 0.5) / SS as f32;
                    let py = y as f32 + (sy as f32 + 0.5) / SS as f32;
                    if !in_squircle(px - origin, py - origin, body, radius) {
                        continue;
                    }
                    // Background: subtle vertical gradient.
                    let t = (py - origin) / body;
                    let base = 0x26 as f32 * (1.0 - t) + 0x14 as f32 * t;
                    let mut col = [base, base, base + 1.0, 255.0];
                    for (cx, cy, r, c) in &dots {
                        let d2 = (px - cx).powi(2) + (py - cy).powi(2);
                        if d2 <= r * r {
                            let a = c[3] as f32 / 255.0;
                            for i in 0..3 {
                                col[i] = col[i] * (1.0 - a) + c[i] as f32 * a;
                            }
                        }
                    }
                    for i in 0..4 {
                        acc[i] += col[i];
                    }
                }
            }
            let n = (SS * SS) as f32;
            let alpha = acc[3] / n;
            if alpha > 0.0 {
                let k = 255.0 / acc[3].max(1.0);
                img.put_pixel(
                    x,
                    y,
                    Rgba([
                        (acc[0] * k).round() as u8,
                        (acc[1] * k).round() as u8,
                        (acc[2] * k).round() as u8,
                        alpha.round() as u8,
                    ]),
                );
            }
        }
    }
    img.save(&out).expect("write icon");
    println!("wrote {out}");
}

fn in_squircle(x: f32, y: f32, size: f32, r: f32) -> bool {
    if x < 0.0 || y < 0.0 || x > size || y > size {
        return false;
    }
    let cx = x.clamp(r, size - r);
    let cy = y.clamp(r, size - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}
