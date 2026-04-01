mod app;
mod render;
mod types;
mod utils;

use app::SpinoramaApp;
use gpui::AppContext as _;
use gpui_ui_kit::{MiniApp, MiniAppConfig};

fn main() {
    MiniApp::run(
        MiniAppConfig::new("Spinorama Viewer")
            .size(1200.0, 800.0)
            .with_theme(true)
            .scrollable(false),
        |cx| cx.new(SpinoramaApp::new),
    );
}
