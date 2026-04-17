use crate::driver::AppDriver;

pub struct QueuePage<'a, 'b> {
    driver: &'a mut AppDriver<'b>,
}

impl<'a, 'b> QueuePage<'a, 'b> {
    pub fn new(driver: &'a mut AppDriver<'b>) -> Self {
        Self { driver }
    }

    pub fn get_queue_length(&mut self) -> usize {
        self.driver.read_app(|app| app.queue_state.len())
    }

    pub fn is_empty(&mut self) -> bool {
        self.get_queue_length() == 0
    }

    pub fn get_current_track_index(&mut self) -> Option<usize> {
        self.driver.read_app(|app| app.playback.current_queue_index)
    }

    pub fn is_playing(&mut self) -> bool {
        self.driver.read_app(|app| app.playback.is_playing)
    }

    pub fn clear_queue(&mut self) {
        self.driver.update_app(|app, _| {
            app.queue_state.clear();
        });
        self.driver.run_until_parked();
    }

    pub fn has_next(&mut self) -> bool {
        let current = self.get_current_track_index();
        let length = self.get_queue_length();
        match current {
            Some(idx) => idx + 1 < length,
            None => length > 0,
        }
    }

    pub fn has_previous(&mut self) -> bool {
        let current = self.get_current_track_index();
        match current {
            Some(idx) => idx > 0,
            None => false,
        }
    }
}
