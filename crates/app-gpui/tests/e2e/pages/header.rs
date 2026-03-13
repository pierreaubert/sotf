use crate::driver::AppDriver;
use sotf_audio_player_gpui::app::{ActiveMenu, Screen};

pub struct HeaderPage<'a, 'b> {
    driver: &'a mut AppDriver<'b>,
}

impl<'a, 'b> HeaderPage<'a, 'b> {
    pub fn new(driver: &'a mut AppDriver<'b>) -> Self {
        Self { driver }
    }

    pub fn get_current_screen(&mut self) -> Screen {
        self.driver.read_app(|app| app.ui_state.current_screen)
    }

    pub fn is_menu_open(&mut self) -> bool {
        self.driver
            .read_app(|app| app.ui_state.active_menu != ActiveMenu::None)
    }

    pub fn get_open_menu(&mut self) -> ActiveMenu {
        self.driver.read_app(|app| app.ui_state.active_menu)
    }

    pub fn navigate_to(&mut self, screen: Screen) {
        self.driver.navigate_to(screen);
    }
}
