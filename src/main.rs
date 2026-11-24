#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use scrivi::icons;
use scrivi::layout;
use scrivi::options;
use scrivi::store;
use scrivi::theme;
use scrivi::ui::ScriviApp;
use scrivi::updates::UpdateController;

fn main() -> eframe::Result {
    #[cfg(windows)]
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([layout::PINNED_WINDOW_WIDTH, layout::PINNED_WINDOW_HEIGHT])
        .with_position([0.0, 0.0])
        .with_always_on_top();
    #[cfg(not(windows))]
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([layout::DEFAULT_WINDOW_WIDTH, layout::DEFAULT_WINDOW_HEIGHT]);
    if let Some(icon) = icons::load_app_icon() {
        viewport = viewport.with_icon(Arc::new(icon));
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Scrivi",
        native_options,
        Box::new(|cc| {
            let app = match store::open_default_db() {
                Ok(db) => {
                    theme::apply_egui_theme(&cc.egui_ctx, db.app_theme());
                    let platform = create_platform(cc);
                    let updates = UpdateController::start(cc.egui_ctx.clone());
                    ScriviApp::new(db, platform, updates)
                }
                Err(err) => {
                    eprintln!("{err}");
                    theme::apply_egui_theme(&cc.egui_ctx, theme::DEFAULT_THEME);
                    ScriviApp::startup_error(err.to_string())
                }
            };
            Ok(Box::new(app))
        }),
    )
}

#[cfg(windows)]
fn create_platform(cc: &eframe::CreationContext<'_>) -> Box<dyn options::WindowPlatform> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let hwnd = cc
        .window_handle()
        .ok()
        .and_then(|handle| match handle.as_raw() {
            RawWindowHandle::Win32(win32) => Some(win32.hwnd.get()),
            _ => None,
        })
        .unwrap_or(0);
    Box::new(options::WindowsPlatform::new(hwnd))
}

#[cfg(not(windows))]
fn create_platform(_cc: &eframe::CreationContext<'_>) -> Box<dyn options::WindowPlatform> {
    Box::new(options::DefaultPlatform)
}
