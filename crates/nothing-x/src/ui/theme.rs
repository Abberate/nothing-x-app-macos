//! Design tokens (Nothing-style monochrome with a red accent) and fonts.

use std::cell::Cell;
use std::sync::Arc;

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub sidebar: Color32,
    pub surface: Color32,
    pub surface_hi: Color32,
    pub stroke: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_faint: Color32,
    /// Inverted chip (white in dark mode, black in light mode) and its content colour.
    pub inverse: Color32,
    pub on_inverse: Color32,
    pub accent: Color32,
    pub good: Color32,
    pub warn: Color32,
    pub overlay: Color32,
}

pub const DARK: Palette = Palette {
    dark: true,
    bg: Color32::from_rgb(0x00, 0x00, 0x00),
    sidebar: Color32::from_rgb(0x0C, 0x0C, 0x0D),
    surface: Color32::from_rgb(0x17, 0x17, 0x19),
    surface_hi: Color32::from_rgb(0x24, 0x24, 0x27),
    stroke: Color32::from_rgb(0x2C, 0x2C, 0x30),
    text: Color32::from_rgb(0xF5, 0xF5, 0xF5),
    text_dim: Color32::from_rgb(0xA1, 0xA1, 0xA6),
    text_faint: Color32::from_rgb(0x63, 0x63, 0x68),
    inverse: Color32::from_rgb(0xF5, 0xF5, 0xF5),
    on_inverse: Color32::from_rgb(0x00, 0x00, 0x00),
    accent: Color32::from_rgb(0xD7, 0x19, 0x21),
    good: Color32::from_rgb(0x2E, 0xC4, 0x6A),
    warn: Color32::from_rgb(0xFF, 0xC4, 0x00),
    overlay: Color32::from_rgba_premultiplied(0, 0, 0, 170),
};

pub const LIGHT: Palette = Palette {
    dark: false,
    bg: Color32::from_rgb(0xF2, 0xF2, 0xF2),
    sidebar: Color32::from_rgb(0xE9, 0xE9, 0xE9),
    surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    surface_hi: Color32::from_rgb(0xEC, 0xEC, 0xEE),
    stroke: Color32::from_rgb(0xDD, 0xDD, 0xE0),
    text: Color32::from_rgb(0x0A, 0x0A, 0x0A),
    text_dim: Color32::from_rgb(0x5F, 0x5F, 0x64),
    text_faint: Color32::from_rgb(0x9A, 0x9A, 0x9F),
    inverse: Color32::from_rgb(0x0A, 0x0A, 0x0A),
    on_inverse: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    accent: Color32::from_rgb(0xD7, 0x19, 0x21),
    good: Color32::from_rgb(0x1F, 0xA8, 0x55),
    warn: Color32::from_rgb(0xE0, 0x9A, 0x00),
    overlay: Color32::from_rgba_premultiplied(0, 0, 0, 90),
};

thread_local! {
    static CURRENT: Cell<Palette> = const { Cell::new(DARK) };
}

pub fn set_palette(p: Palette) {
    CURRENT.with(|c| c.set(p));
}

/// Palette for the frame being drawn.
pub fn pal() -> Palette {
    CURRENT.with(|c| c.get())
}

const REGULAR_TTF: &[u8] = include_bytes!("../../assets/fonts/SpaceGrotesk-400.ttf");
const MEDIUM_TTF: &[u8] = include_bytes!("../../assets/fonts/SpaceGrotesk-500.ttf");

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for (name, data) in [("sg-regular", REGULAR_TTF), ("sg-medium", MEDIUM_TTF)] {
        fonts
            .font_data
            .insert(name.into(), Arc::new(FontData::from_static(data)));
    }
    // Regular is the body face; egui's own fonts stay as fallbacks (emoji, other scripts).
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .expect("proportional family")
        .insert(0, "sg-regular".into());
    let fallbacks = fonts.families[&FontFamily::Proportional].clone();
    let mut list = vec!["sg-medium".to_string()];
    list.extend(fallbacks.iter().skip(1).cloned());
    fonts
        .families
        .insert(FontFamily::Name("medium".into()), list);
    ctx.set_fonts(fonts);
}

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("medium".into()))
}
