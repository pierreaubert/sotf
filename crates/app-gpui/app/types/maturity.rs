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
	    | Screen::Spinorama
	    | Screen::Studio
	    | Screen::RoomEq
		=> ReleaseChannel::Prod,
            Screen::PluginGraph => ReleaseChannel::Alpha,
        }
    }
}
