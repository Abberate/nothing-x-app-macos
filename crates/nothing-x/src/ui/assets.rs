//! Product renders, decoded on demand and cropped to their visible pixels.

use egui::{Color32, ColorImage, Painter, Rect, TextureHandle, TextureOptions, Vec2, pos2, vec2};

use super::images_table::IMAGES;

pub const FALLBACK_KEY: &str = "b172_black";

#[derive(Clone)]
pub struct Tex {
    pub handle: TextureHandle,
    pub size: Vec2,
}

impl Tex {
    /// Paints the image scaled to fit `rect` (aspect preserved), centred.
    pub fn paint_fit(&self, p: &Painter, rect: Rect, tint: Color32) -> Rect {
        let scale = (rect.width() / self.size.x).min(rect.height() / self.size.y);
        let r = Rect::from_center_size(rect.center(), self.size * scale);
        p.image(
            self.handle.id(),
            r,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            tint,
        );
        r
    }
}

pub struct BudImages {
    pub key: String,
    pub left: Option<Tex>,
    pub right: Option<Tex>,
    pub case: Option<Tex>,
}

impl BudImages {
    /// The asset key to use for a model image key (falls back to a known render).
    pub fn resolve_key(key: Option<&str>) -> &str {
        match key {
            Some(k)
                if IMAGES
                    .iter()
                    .any(|(n, _)| n.strip_suffix("_left") == Some(k)) =>
            {
                k
            }
            _ => FALLBACK_KEY,
        }
    }

    pub fn load(ctx: &egui::Context, key: &str) -> Self {
        let key = Self::resolve_key(Some(key));
        Self {
            key: key.to_string(),
            left: load(ctx, &format!("{key}_left")),
            right: load(ctx, &format!("{key}_right")),
            case: load(ctx, &format!("{key}_case")),
        }
    }
}

fn load(ctx: &egui::Context, name: &str) -> Option<Tex> {
    let (_, bytes) = IMAGES.iter().find(|(n, _)| *n == name)?;
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .ok()?
        .to_rgba8();
    let (w, h) = img.dimensions();
    // Crop fully transparent margins so layouts align on the visible product.
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0u32, 0u32);
    for (x, y, px) in img.enumerate_pixels() {
        if px[3] > 10 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x1 < x0 || y1 < y0 {
        return None;
    }
    let cropped = image::imageops::crop_imm(&img, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image();
    let size = [cropped.width() as usize, cropped.height() as usize];
    let color = ColorImage::from_rgba_unmultiplied(size, cropped.as_raw());
    let handle = ctx.load_texture(name, color, TextureOptions::LINEAR);
    Some(Tex {
        handle,
        size: vec2(size[0] as f32, size[1] as f32),
    })
}
