use sotf_audio_player::ReleaseChannel;

use super::Screen;

impl Screen {
    /// Returns the maturity level of this screen.
    pub fn maturity(&self) -> ReleaseChannel {
        match self {
            Screen::Library
            | Screen::Queue
            | Screen::Spectrum
            | Screen::Settings
            | Screen::Recording
            | Screen::HeadphoneEq
            | Screen::Spinorama => ReleaseChannel::Prod,

            Screen::RoomEq => ReleaseChannel::Beta,

            Screen::Studio | Screen::PluginGraph => ReleaseChannel::Alpha,
        }
    }
}
