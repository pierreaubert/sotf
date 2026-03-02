use super::app_impl::App;

impl App {
    pub fn increase_volume(&mut self) {
        self.volume = (self.volume + 0.05).min(1.0);
    }

    pub fn decrease_volume(&mut self) {
        self.volume = (self.volume - 0.05).max(0.0);
    }
}
