//! Minimal GPUI hello world — diagnose text rendering.
//!
//! Run with: cargo run -p sotf-gpui --example hello_world

use gpui::*;
use rust_embed::RustEmbed;
use std::borrow::Cow;

/// Embedded assets (reuses the same assets folder)
#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "fonts/*.ttf"]
struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }
        Self::get(path)
            .map(|file| Some(file.data))
            .ok_or_else(|| anyhow::anyhow!("Asset not found: {}", path))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| {
                if p.starts_with(path) {
                    Some(SharedString::from(p.to_string()))
                } else {
                    None
                }
            })
            .collect())
    }
}

/// Diagnostic view — tests multiple text rendering approaches
struct HelloView;

impl Render for HelloView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Hardcode everything — no theme dependency
        let white = rgb(0xffffff);
        let red = rgb(0xff0000);
        let green = rgb(0x00ff00);
        let blue = rgb(0x4488ff);
        let dark_bg = rgb(0x1a1a1a);

        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_8()
            .bg(dark_bg)
            .text_color(white)
            .text_size(px(16.0))
            // Test 1: Plain string child, no font_family set
            .child(
                div()
                    .bg(rgb(0x333333))
                    .p_2()
                    .child("Test 1: plain text, no font_family, hardcoded white on gray"),
            )
            // Test 2: With explicit font_family "Helvetica"
            .child(
                div()
                    .bg(rgb(0x333333))
                    .p_2()
                    .font_family("Helvetica")
                    .child("Test 2: Helvetica font"),
            )
            // Test 3: With .SystemUI font
            .child(
                div()
                    .bg(rgb(0x333333))
                    .p_2()
                    .font_family(".AppleSystemUIFont")
                    .child("Test 3: .AppleSystemUIFont"),
            )
            // Test 4: Colored text on colored background
            .child(
                div()
                    .bg(rgb(0x000000))
                    .p_2()
                    .text_color(red)
                    .text_size(px(24.0))
                    .child("Test 4: RED text 24px on black bg"),
            )
            // Test 5: Green text, large
            .child(
                div()
                    .bg(rgb(0x000000))
                    .p_2()
                    .text_color(green)
                    .text_size(px(32.0))
                    .child("Test 5: GREEN 32px"),
            )
            // Test 6: B612 (loaded custom font)
            .child(
                div()
                    .bg(rgb(0x333333))
                    .p_2()
                    .font_family("B612")
                    .text_color(blue)
                    .text_size(px(28.0))
                    .child("Test 6: B612 28px BLUE"),
            )
            // Test 7: A colored box (no text) to confirm rendering works at all
            .child(
                div()
                    .w(px(200.0))
                    .h(px(40.0))
                    .bg(rgb(0xff6600))
                    .rounded_md(),
            )
    }
}

fn main() {
    gpui::Application::with_platform(std::rc::Rc::new(gpui_macos::MacPlatform::new(false)))
        .with_assets(Assets)
        .run(move |cx| {
            // Load the B612 font
            let font_paths = [
                "fonts/B612-Regular.ttf",
                "fonts/B612-Italic.ttf",
                "fonts/B612-Bold.ttf",
                "fonts/B612-BoldItalic.ttf",
            ];
            let font_data: Vec<Cow<'static, [u8]>> = font_paths
                .iter()
                .filter_map(|p| Assets::get(p).map(|f| f.data))
                .collect();
            if !font_data.is_empty() {
                if let Err(e) = cx.text_system().add_fonts(font_data) {
                    eprintln!("FONT LOAD ERROR: {e}");
                } else {
                    eprintln!("Fonts loaded OK ({} files)", font_paths.len());
                }
            } else {
                eprintln!("NO FONT DATA FOUND");
            }

            let _ = cx.open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("Hello GPUI - Text Diagnostic".into()),
                        appears_transparent: false,
                        traffic_light_position: None,
                    }),
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::new(px(200.0), px(200.0)),
                        size: Size {
                            width: px(700.0),
                            height: px(500.0),
                        },
                    })),
                    window_background: WindowBackgroundAppearance::Opaque,
                    focus: true,
                    show: true,
                    kind: WindowKind::Normal,
                    is_movable: true,
                    display_id: None,
                    is_minimizable: true,
                    is_resizable: true,
                    app_id: None,
                    tabbing_identifier: None,
                    window_decorations: None,
                    window_min_size: None,
                },
                |_window, cx| cx.new(|_| HelloView),
            );
        });
}
