//! Nothing X App for macOS (unofficial) — companion for Nothing & CMF earbuds.

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(nothing_x::ui::APP_NAME)
            .with_app_id("io.github.nothingx-mac")
            .with_inner_size([1040.0, 760.0])
            .with_min_inner_size([760.0, 560.0])
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false)
            .with_titlebar_buttons_shown(true),
        ..Default::default()
    };
    eframe::run_native(
        nothing_x::ui::APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(nothing_x::ui::NothingApp::new(cc)))),
    )
}
