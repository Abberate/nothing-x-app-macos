//! Nothing X App for macOS (unofficial) — companion for Nothing & CMF earbuds.

/// Default window size; `NOTHINGX_WINDOW=WxH` overrides it (visual checks).
fn window_size() -> [f32; 2] {
    std::env::var("NOTHINGX_WINDOW")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some([w.parse().ok()?, h.parse().ok()?])
        })
        .unwrap_or([1040.0, 760.0])
}

fn test_run() -> bool {
    std::env::args().any(|a| a == "--demo")
        || std::env::vars_os().any(|(k, _)| k.to_string_lossy().starts_with("NOTHINGX_"))
}

/// Dock and app switcher icon.
///
/// Without one, eframe replaces the Dock icon with its default egui logo at launch.
/// Inside the .app bundle we hand it an empty icon so it leaves the Dock alone and
/// macOS shows the bundle's AppIcon.icns, exactly as Finder and Launchpad do.
/// A bare `cargo run` binary has no bundle icon, so it gets the same artwork from the PNG.
fn app_icon() -> egui::IconData {
    let bundled = std::env::current_exe()
        .map(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false);
    if bundled {
        return egui::IconData::default();
    }
    image::load_from_memory(include_bytes!("../../../packaging/icon_1024.png"))
        .map(|img| {
            let img = img.into_rgba8();
            let (width, height) = img.dimensions();
            egui::IconData {
                rgba: img.into_raw(),
                width,
                height,
            }
        })
        .unwrap_or_default()
}

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    if let Some(out) = std::env::var_os("NOTHINGX_DMG_BG") {
        // Installer artwork (see scripts/make_dmg.sh).
        let size = nothing_x::ui::dmg_bg::SIZE;
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size(size)
                .with_resizable(false)
                .with_decorations(false)
                .with_icon(egui::IconData::default()),
            persistence_path: Some(std::env::temp_dir().join("nothing-x-app-test.ron")),
            ..Default::default()
        };
        return eframe::run_native(
            "Nothing X App installer art",
            options,
            Box::new(move |cc| {
                Ok(Box::new(nothing_x::ui::dmg_bg::DmgBackground::new(
                    cc,
                    out.into(),
                )))
            }),
        );
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(nothing_x::ui::APP_NAME)
            .with_app_id("io.github.nothingx-mac")
            .with_icon(app_icon())
            .with_inner_size(window_size())
            .with_min_inner_size([760.0, 560.0])
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false)
            .with_titlebar_buttons_shown(true)
            // Visual checks only: open directly in fullscreen.
            .with_fullscreen(std::env::var_os("NOTHINGX_FULLSCREEN").is_some()),
        // Demo / visual-check runs get their own throw-away storage, so they never
        // touch (nor get resized by) the user's saved window and preferences.
        persistence_path: test_run().then(|| std::env::temp_dir().join("nothing-x-app-test.ron")),
        ..Default::default()
    };
    eframe::run_native(
        nothing_x::ui::APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(nothing_x::ui::NothingApp::new(cc)))),
    )
}
