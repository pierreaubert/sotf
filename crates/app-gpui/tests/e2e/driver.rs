use crate::pages::headphone::HeadphoneEqPage;
use crate::pages::spinorama::SpinoramaPage;
use gpui::{Context, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::{App, AppState, Screen, SettingsTab};
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

/// High-level driver for the application in E2E tests.
pub struct AppDriver<'a> {
    pub cx: &'a mut VisualTestContext,
    pub view: WindowHandle<PlayerView>,
}

impl<'a> AppDriver<'a> {
    pub fn new(cx: &'a mut VisualTestContext, view: WindowHandle<PlayerView>) -> Self {
        Self { cx, view }
    }

    /// Access the underlying App state
    pub fn update_app<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut App, &mut Context<AppState>) -> R,
    {
        self.view
            .update(self.cx, |view, _window, cx| {
                view.state.update(cx, |state, cx| f(&mut state.app, cx))
            })
            .unwrap()
    }

    /// Read the underlying App state
    /// Note: Requires &mut self because standard view access in tests via update() requires mutable context.
    pub fn read_app<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&App) -> R,
    {
        self.view
            .update(self.cx, |view, _window, cx| {
                let state = view.state.read(cx);
                f(&state.app)
            })
            .unwrap()
    }

    /// Open a specific settings tab
    pub fn open_settings_tab(&mut self, tab: SettingsTab) {
        self.update_app(|app, _| {
            app.ui_state.current_screen = Screen::Settings;
            app.ui_state.active_settings_tab = tab;
        });
        self.cx.run_until_parked();
    }

    /// Navigate to a screen
    pub fn navigate_to(&mut self, screen: Screen) {
        self.update_app(|app, _| {
            app.ui_state.current_screen = screen;
        });
        self.cx.run_until_parked();
    }

    /// Verify a toast message is shown
    pub fn expect_toast(&mut self, message_part: &str) -> Result<(), Box<dyn Error>> {
        self.view
            .update(self.cx, |view, _window, cx| {
                let state = view.state.read(cx);
                if let Some(toast) = &state.app.ui_state.toast_message
                    && toast.message.contains(message_part)
                {
                    return Ok(());
                }
                Err(format!(
                    "Expected toast containing '{}', found '{}'",
                    message_part,
                    state
                        .app
                        .ui_state
                        .toast_message
                        .as_ref()
                        .map(|t| t.message.as_str())
                        .unwrap_or("None")
                )
                .into())
            })
            .unwrap() /* Unwrap GPUI context error */
    }
    pub fn spinorama<'s>(&'s mut self) -> SpinoramaPage<'s, 'a> {
        SpinoramaPage::new(self)
    }

    pub fn headphone_eq<'s>(&'s mut self) -> HeadphoneEqPage<'s, 'a> {
        HeadphoneEqPage::new(self)
    }

    pub fn simulate_keystrokes(&mut self, keystrokes: &str) {
        self.cx.simulate_keystrokes(keystrokes);
    }

    pub fn run_until_parked(&mut self) {
        self.cx.run_until_parked();
    }
}
