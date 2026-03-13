use crate::driver::AppDriver;
use sotf_audio_player_gpui::app::InputMode;

pub struct DialogsPage<'a, 'b> {
    driver: &'a mut AppDriver<'b>,
}

impl<'a, 'b> DialogsPage<'a, 'b> {
    pub fn new(driver: &'a mut AppDriver<'b>) -> Self {
        Self { driver }
    }

    pub fn is_about_dialog_open(&mut self) -> bool {
        self.driver
            .read_app(|app| app.ui_state.input_mode == InputMode::About)
    }

    pub fn is_shortcuts_dialog_open(&mut self) -> bool {
        self.driver
            .read_app(|app| app.ui_state.input_mode == InputMode::KeyboardShortcuts)
    }

    pub fn has_toast(&mut self) -> bool {
        self.driver
            .read_app(|app| app.ui_state.toast_message.is_some())
    }

    pub fn get_toast_message(&mut self) -> Option<String> {
        self.driver.read_app(|app| {
            app.ui_state
                .toast_message
                .as_ref()
                .map(|t| t.message.clone())
        })
    }

    pub fn has_active_menu(&mut self) -> bool {
        self.driver.read_app(|app| {
            app.ui_state.active_menu != sotf_audio_player_gpui::app::ActiveMenu::None
        })
    }

    pub fn close_dialogs(&mut self) {
        self.driver.update_app(|app, _| {
            app.ui_state.input_mode = InputMode::Normal;
            app.ui_state.active_menu = sotf_audio_player_gpui::app::ActiveMenu::None;
        });
        self.driver.run_until_parked();
    }
}
