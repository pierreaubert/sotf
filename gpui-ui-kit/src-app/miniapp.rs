//! MiniApp - A minimal application template for GPUI examples and showcases
//!
//! Provides a reusable application shell with:
//! - Standard menu bar with Quit option (Cmd+Q on macOS)
//! - Configurable window title and size
//! - Extensible for additional default features
//!
//! # Example
//!
//! ```ignore
//! use gpui::*;
//! use gpui_ui_kit::miniapp::{MiniApp, MiniAppConfig};
//!
//! struct MyDemo;
//!
//! impl Render for MyDemo {
//!     fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
//!         div().child("Hello from MiniApp!")
//!     }
//! }
//!
//! fn main() {
//!     MiniApp::run(MiniAppConfig::new("My Demo"), |cx| cx.new(|_| MyDemo));
//! }
//! ```

use gpui::*;

/// Configuration for a MiniApp instance
#[derive(Clone)]
pub struct MiniAppConfig {
    /// Window title
    pub title: SharedString,
    /// Window width in pixels
    pub width: f32,
    /// Window height in pixels
    pub height: f32,
    /// Application name shown in menu bar
    pub app_name: SharedString,
}

impl MiniAppConfig {
    /// Create a new configuration with the given title
    ///
    /// Uses default window size of 900x700 pixels.
    pub fn new(title: impl Into<SharedString>) -> Self {
        let title = title.into();
        Self {
            title: title.clone(),
            width: 900.0,
            height: 700.0,
            app_name: title,
        }
    }

    /// Set the window size
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Set the application name shown in the menu bar
    ///
    /// By default, this is the same as the window title.
    pub fn app_name(mut self, name: impl Into<SharedString>) -> Self {
        self.app_name = name.into();
        self
    }
}

impl Default for MiniAppConfig {
    fn default() -> Self {
        Self::new("MiniApp")
    }
}

// Define the Quit action for the menu
actions!(miniapp, [Quit]);

/// MiniApp provides a minimal application shell for GPUI examples and showcases
///
/// It handles:
/// - Application lifecycle
/// - Standard menu bar with Quit option
/// - Window creation with configurable size
/// - Keyboard shortcut binding (Cmd+Q to quit)
pub struct MiniApp;

impl MiniApp {
    /// Run a MiniApp with the given configuration and view builder
    ///
    /// The `build_view` closure receives a `&mut Context<V>` and should return
    /// a `V` instance that implements `Render`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use gpui::*;
    /// use gpui_ui_kit::MiniApp;
    ///
    /// struct MyView;
    /// impl Render for MyView {
    ///     fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    ///         div().child("Hello!")
    ///     }
    /// }
    ///
    /// MiniApp::run(MiniAppConfig::new("Demo"), |cx| cx.new(MyView::new));
    /// ```
    pub fn run<V, F>(config: MiniAppConfig, build_view: F)
    where
        V: Render + 'static,
        F: FnOnce(&mut App) -> Entity<V> + 'static,
    {
        let config_clone = config.clone();

        Application::new().run(move |cx: &mut App| {
            // Register quit action
            cx.on_action::<Quit>(|_action, cx| {
                cx.quit();
            });

            // Set up menu bar with application name
            let quit_label: SharedString = format!("Quit {}", config_clone.app_name).into();
            cx.set_menus(vec![Menu {
                name: config_clone.app_name.clone(),
                items: vec![MenuItem::action(quit_label, Quit)],
            }]);

            // Bind Cmd+Q to quit
            cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);

            // Create window
            let bounds = Bounds::centered(
                None,
                size(px(config_clone.width), px(config_clone.height)),
                cx,
            );

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some(config_clone.title.clone()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| build_view(cx),
            )
            .unwrap();

            cx.activate(true);
        });
    }

    /// Run a MiniApp with default configuration
    ///
    /// Uses "MiniApp" as the default title and 900x700 window size.
    pub fn run_default<V, F>(build_view: F)
    where
        V: Render + 'static,
        F: FnOnce(&mut App) -> Entity<V> + 'static,
    {
        Self::run(MiniAppConfig::default(), build_view);
    }
}

#[cfg(test)]
mod tests {
    use super::MiniAppConfig;

    #[test]
    fn test_config_new() {
        let config = MiniAppConfig::new("Test App");
        assert_eq!(config.title.as_ref(), "Test App");
        assert_eq!(config.app_name.as_ref(), "Test App");
        assert_eq!(config.width, 900.0);
        assert_eq!(config.height, 700.0);
    }

    #[test]
    fn test_config_size() {
        let config = MiniAppConfig::new("Test").size(1200.0, 800.0);
        assert_eq!(config.width, 1200.0);
        assert_eq!(config.height, 800.0);
    }

    #[test]
    fn test_config_app_name() {
        let config = MiniAppConfig::new("Window Title").app_name("Menu Name");
        assert_eq!(config.title.as_ref(), "Window Title");
        assert_eq!(config.app_name.as_ref(), "Menu Name");
    }

    #[test]
    fn test_config_default() {
        let config = MiniAppConfig::default();
        assert_eq!(config.title.as_ref(), "MiniApp");
    }

    #[test]
    fn test_config_builder_chain() {
        let config = MiniAppConfig::new("Demo")
            .size(1000.0, 600.0)
            .app_name("My Demo App");

        assert_eq!(config.title.as_ref(), "Demo");
        assert_eq!(config.width, 1000.0);
        assert_eq!(config.height, 600.0);
        assert_eq!(config.app_name.as_ref(), "My Demo App");
    }
}
