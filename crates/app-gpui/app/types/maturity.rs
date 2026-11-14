use sotf_audio_player::ReleaseChannel;

use super::Screen;

impl Screen {
    /// Returns the maturity level of this screen.
    #[must_use]
    pub fn maturity(&self) -> ReleaseChannel {
        match self {
            Screen::Home
            | Screen::HomeShelf
            | Screen::Library
            | Screen::NowPlaying
            | Screen::Queue
            | Screen::Spectrum
            | Screen::Settings
            | Screen::SettingsDetail
            | Screen::StudioHub
            | Screen::EqCurve
            | Screen::Recording
            | Screen::HeadphoneEq
            | Screen::Spinorama
            | Screen::ListeningTest
            | Screen::Studio
            | Screen::RoomEq
            | Screen::Streams
            | Screen::PluginGraph
            | Screen::Playlists => ReleaseChannel::Prod,
        }
    }
}
