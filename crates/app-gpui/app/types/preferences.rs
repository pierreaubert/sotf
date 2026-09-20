use super::{Screen, SettingsTab};

/// Presentation categories preserve the existing setting identities and routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferencesCategory {
    Audio,
    Library,
    Appearance,
    Keyboard,
    Plugins,
    Connections,
    Advanced,
}

impl PreferencesCategory {
    pub const ALL: [Self; 7] = [
        Self::Audio,
        Self::Library,
        Self::Appearance,
        Self::Keyboard,
        Self::Plugins,
        Self::Connections,
        Self::Advanced,
    ];

    pub fn for_tab(tab: SettingsTab) -> Self {
        match tab {
            SettingsTab::AudioDevice => Self::Audio,
            SettingsTab::Library | SettingsTab::Metadata => Self::Library,
            SettingsTab::Theme | SettingsTab::Language => Self::Appearance,
            SettingsTab::Keybindings => Self::Keyboard,
            SettingsTab::Misc => Self::Plugins,
            SettingsTab::Federation | SettingsTab::Servers => Self::Connections,
            SettingsTab::ReleaseChannel => Self::Advanced,
        }
    }

    pub fn tabs(self) -> &'static [SettingsTab] {
        match self {
            Self::Audio => &[SettingsTab::AudioDevice],
            Self::Library => &[SettingsTab::Library, SettingsTab::Metadata],
            Self::Appearance => &[SettingsTab::Theme, SettingsTab::Language],
            Self::Keyboard => &[SettingsTab::Keybindings],
            Self::Plugins => &[SettingsTab::Misc],
            Self::Connections => &[SettingsTab::Federation, SettingsTab::Servers],
            Self::Advanced => &[SettingsTab::ReleaseChannel],
        }
    }

    pub fn visible_tabs(self, is_ios: bool) -> Vec<SettingsTab> {
        let visible = SettingsTab::visible_tabs_for_ios(is_ios);
        self.tabs()
            .iter()
            .copied()
            .filter(|tab| visible.contains(tab))
            .collect()
    }

    pub fn keywords(tab: SettingsTab) -> &'static str {
        match tab {
            SettingsTab::AudioDevice => {
                "audio playback output input device sound speaker headphones sample rate channels buffer latency systemwide wireless cast replaygain normalization loudness"
            }
            SettingsTab::Library => {
                "library music folder directory scan threads replaygain loudness waveform similarity database analysis"
            }
            SettingsTab::Metadata => "metadata musicbrainz artist album tags account",
            SettingsTab::Theme => {
                "appearance theme dark light system schedule custom colors text font zoom size bounds motion"
            }
            SettingsTab::Language => "appearance language locale translation",
            SettingsTab::Keybindings => "keyboard shortcut hotkey binding keys conflict reset",
            SettingsTab::Misc => {
                "plugins vst clap au scan discovery worker validation sandbox cpu cores"
            }
            SettingsTab::Federation => {
                "connections music sources accounts remote library provider spotify tidal authentication login"
            }
            SettingsTab::Servers => {
                "connections players sharing server api mpd dlna port tls password token address network"
            }
            SettingsTab::ReleaseChannel => {
                "advanced release channel maturity experimental beta feature availability"
            }
        }
    }

    pub fn matches(tab: SettingsTab, query: &str, translated_label: &str) -> bool {
        let haystack = format!("{} {}", Self::keywords(tab), translated_label).to_lowercase();
        query
            .split_whitespace()
            .all(|word| haystack.contains(&word.to_lowercase()))
    }
}

#[derive(Debug, Clone, Default)]
pub struct PreferencesNavigation {
    pub close_pending: bool,
    pub category_open: bool,
    pub category_highlight: Option<usize>,
    pub query: String,
    pub return_screen: Option<Screen>,
    pub setting: Option<PreferencesSetting>,
    pub setting_focus: Option<gpui::FocusHandle>,
    pub reveal_setting: bool,
}

impl PreferencesNavigation {
    pub fn clear_setting_target(&mut self) {
        self.setting = None;
        self.setting_focus = None;
        self.reveal_setting = false;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferencesSetting {
    TextSize,
    MinimumFont,
    MaximumFont,
    ScanWorkers,
    CpuLimit,
    ReduceMotion,
    Language,
    ThemeMode,
    PluginDiscovery,
    MetadataSearch,
    FeatureAvailability,
    OutputDevice,
    ReplayGain,
    MusicFolders,
    AudioAnalysis,
    Shortcuts,
    RemoteSources,
    RemotePlayers,
    SharingServers,
    AudioSource,
    SystemwideSampleRate,
    SystemwideChannels,
    SystemwideBuffer,
}

impl PreferencesSetting {
    pub const ALL: [Self; 23] = [
        Self::TextSize,
        Self::MinimumFont,
        Self::MaximumFont,
        Self::ScanWorkers,
        Self::CpuLimit,
        Self::ReduceMotion,
        Self::Language,
        Self::ThemeMode,
        Self::PluginDiscovery,
        Self::MetadataSearch,
        Self::FeatureAvailability,
        Self::OutputDevice,
        Self::ReplayGain,
        Self::MusicFolders,
        Self::AudioAnalysis,
        Self::Shortcuts,
        Self::RemoteSources,
        Self::RemotePlayers,
        Self::SharingServers,
        Self::AudioSource,
        Self::SystemwideSampleRate,
        Self::SystemwideChannels,
        Self::SystemwideBuffer,
    ];

    pub fn tab(self) -> SettingsTab {
        match self {
            Self::TextSize | Self::MinimumFont | Self::MaximumFont => SettingsTab::Theme,
            Self::ScanWorkers => SettingsTab::Library,
            Self::CpuLimit => SettingsTab::Misc,
            Self::ReduceMotion => SettingsTab::Theme,
            Self::Language => SettingsTab::Language,
            Self::ThemeMode => SettingsTab::Theme,
            Self::PluginDiscovery => SettingsTab::Misc,
            Self::MetadataSearch => SettingsTab::Metadata,
            Self::FeatureAvailability => SettingsTab::ReleaseChannel,
            Self::OutputDevice => SettingsTab::AudioDevice,
            Self::ReplayGain => SettingsTab::AudioDevice,
            Self::MusicFolders | Self::AudioAnalysis => SettingsTab::Library,
            Self::Shortcuts => SettingsTab::Keybindings,
            Self::RemoteSources | Self::RemotePlayers => SettingsTab::Federation,
            Self::SharingServers => SettingsTab::Servers,
            Self::AudioSource
            | Self::SystemwideSampleRate
            | Self::SystemwideChannels
            | Self::SystemwideBuffer => SettingsTab::AudioDevice,
        }
    }

    pub fn is_systemwide_format(self) -> bool {
        matches!(
            self,
            Self::SystemwideSampleRate | Self::SystemwideChannels | Self::SystemwideBuffer
        )
    }

    pub fn is_available(self) -> bool {
        if self == Self::AudioSource || self.is_systemwide_format() {
            return cfg!(all(target_os = "macos", feature = "hal"));
        }
        self != Self::PluginDiscovery
            || cfg!(any(
                target_os = "linux",
                target_os = "macos",
                target_os = "windows"
            ))
    }

    pub fn label(self, language: crate::app::i18n::Language) -> &'static str {
        match self {
            Self::SystemwideSampleRate => {
                crate::app::i18n::AudioPreferencesPersistenceTranslations::systemwide_format_labels(
                    language,
                )[0]
            }
            Self::SystemwideChannels => {
                crate::app::i18n::AudioPreferencesPersistenceTranslations::systemwide_format_labels(
                    language,
                )[1]
            }
            Self::SystemwideBuffer => {
                crate::app::i18n::AudioPreferencesPersistenceTranslations::systemwide_format_labels(
                    language,
                )[2]
            }
            Self::AudioSource => {
                crate::app::i18n::AudioDeviceTranslations::for_language(language).audio_source
            }
            Self::RemoteSources => {
                crate::app::i18n::FederationTranslations::for_language(language).streaming
            }
            Self::RemotePlayers => {
                crate::app::i18n::ServerSettingsTranslations::for_language(language).remote_players
            }
            Self::SharingServers => {
                crate::app::i18n::ServerSettingsTranslations::for_language(language).serves_media
            }
            Self::Shortcuts => {
                crate::app::i18n::KeybindingTranslations::for_language(language).customize
            }
            Self::ReplayGain => {
                crate::app::i18n::Translations::for_language(language).settings_enable_replaygain
            }
            Self::MusicFolders => {
                crate::app::i18n::Translations::for_language(language).settings_managed_directories
            }
            Self::AudioAnalysis => {
                crate::app::i18n::Translations::for_language(language).settings_audio_analysis
            }
            Self::OutputDevice => {
                crate::app::i18n::Translations::for_language(language).devices_title
            }
            Self::FeatureAvailability => {
                crate::app::i18n::Translations::for_language(language)
                    .settings_release_channel_title
            }
            Self::MetadataSearch => {
                crate::app::i18n::SettingsSurfaceTranslations::for_language(language)
                    .metadata_services
            }
            Self::TextSize | Self::MinimumFont | Self::MaximumFont => {
                crate::app::i18n::DesktopTranslations::typography_settings(language)[self as usize]
            }
            Self::ScanWorkers => {
                crate::app::i18n::SettingsSurfaceTranslations::for_language(language)
                    .scanner_threads
            }
            Self::CpuLimit => {
                crate::app::i18n::SettingsSurfaceTranslations::for_language(language).max_cpu_cores
            }
            Self::ReduceMotion => {
                crate::app::i18n::AppearanceTranslations::for_language(language).reduce_motion
            }
            Self::Language => {
                crate::app::i18n::Translations::for_language(language).settings_language
            }
            Self::ThemeMode => {
                crate::app::i18n::Translations::for_language(language).settings_theme
            }
            Self::PluginDiscovery => {
                crate::app::i18n::SettingsSurfaceTranslations::for_language(language)
                    .external
                    .title
            }
        }
    }

    pub fn matches(self, query: &str, language: crate::app::i18n::Language) -> bool {
        let keywords = match self {
            Self::SystemwideSampleRate => {
                "systemwide input sample rate sampling frequency hz khz hal audio format"
            }
            Self::SystemwideChannels => {
                "systemwide input channels channel count stereo surround hal audio format"
            }
            Self::SystemwideBuffer => {
                "systemwide input buffer size frames latency hal audio format"
            }
            Self::AudioSource => "audio source systemwide input file player hal",
            Self::RemoteSources => {
                "connections remote music libraries sources provider accounts streaming subsonic tidal spotify"
            }
            Self::RemotePlayers => {
                "connections remote playback devices players listening room name"
            }
            Self::SharingServers => {
                "connections sharing servers sotf api mpd dlna tls authentication"
            }
            Self::Shortcuts => {
                "playback keyboard shortcuts keybindings bindings keys hotkeys play pause customize"
            }
            Self::ReplayGain => "audio replaygain loudness playback normalization album track",
            Self::MusicFolders => "library music folders directories add scanning collection",
            Self::AudioAnalysis => {
                "library audio analysis waveform waveforms similarity bliss compute"
            }
            Self::OutputDevice => "audio playback output device speakers headphones dac sound card",
            Self::FeatureAvailability => {
                "advanced feature availability maturity release channel stable beta alpha experimental"
            }
            Self::MetadataSearch => "library musicbrainz metadata search provider account",
            Self::TextSize => "text size interface scale zoom typography appearance",
            Self::MinimumFont => "minimum min font size typography limits appearance",
            Self::MaximumFont => "maximum max font size typography limits appearance",
            Self::ScanWorkers => "library scan scanner workers threads parallel scanning",
            Self::CpuLimit => "plugins maximum max cpu cores limit processors runtime",
            Self::ReduceMotion => "appearance reduce motion accessibility animation transitions",
            Self::Language => "appearance language locale translation",
            Self::ThemeMode => "appearance theme mode light dark system scheduled automatic",
            Self::PluginDiscovery => "external plugins discovery scan vst vst3 clap au audio units",
        };
        let haystack = format!("{} {keywords}", self.label(language)).to_lowercase();
        query
            .split_whitespace()
            .all(|word| haystack.contains(&word.to_lowercase()))
    }
}
