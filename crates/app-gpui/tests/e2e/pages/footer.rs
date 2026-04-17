use crate::driver::AppDriver;

pub struct FooterPage<'a, 'b> {
    driver: &'a mut AppDriver<'b>,
}

impl<'a, 'b> FooterPage<'a, 'b> {
    pub fn new(driver: &'a mut AppDriver<'b>) -> Self {
        Self { driver }
    }

    pub fn is_playing(&mut self) -> bool {
        self.driver.read_app(|app| app.playback.is_playing)
    }

    pub fn get_volume(&mut self) -> f32 {
        self.driver.read_app(|app| app.playback.volume)
    }

    pub fn get_current_track_title(&mut self) -> Option<String> {
        self.driver.read_app(|app| {
            app.queue_state
                .current_track()
                .map(|t| t.title.clone().unwrap_or_else(|| "Unknown".to_string()))
        })
    }

    pub fn is_muted(&mut self) -> bool {
        self.driver.read_app(|app| app.playback.muted)
    }

    pub fn get_playback_position(&mut self) -> f64 {
        self.driver.read_app(|app| app.playback.position_secs)
    }
}
