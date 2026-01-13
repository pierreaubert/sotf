use crate::driver::AppDriver;

pub struct SettingsPage<'a, 'b> {
    driver: &'a mut AppDriver<'b>,
}

impl<'a, 'b> SettingsPage<'a, 'b> {
    pub fn new(driver: &'a mut AppDriver<'b>) -> Self {
        Self { driver }
    }
}
