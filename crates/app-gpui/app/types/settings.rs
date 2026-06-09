/// Settings screen tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Library,
    Theme,
    Language,
    Keybindings,
    AudioDevice,
    Misc,
    Federation,
    Servers,
    ReleaseChannel,
}

impl SettingsTab {
    pub const ALL: [SettingsTab; 9] = [
        SettingsTab::Library,
        SettingsTab::Theme,
        SettingsTab::Language,
        SettingsTab::Keybindings,
        SettingsTab::AudioDevice,
        SettingsTab::Misc,
        SettingsTab::Federation,
        SettingsTab::Servers,
        SettingsTab::ReleaseChannel,
    ];

    pub fn visible_tabs() -> Vec<SettingsTab> {
        Self::visible_tabs_for_ios(cfg!(target_os = "ios"))
    }

    pub fn visible_tabs_for_ios(is_ios: bool) -> Vec<SettingsTab> {
        Self::ALL
            .into_iter()
            .filter(|tab| {
                !(is_ios && matches!(tab, SettingsTab::Library | SettingsTab::Keybindings))
            })
            .collect()
    }

    pub fn fallback_for_platform() -> SettingsTab {
        SettingsTab::Servers
    }
}

/// Type of scan operation that can show a progress modal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanType {
    /// Library scan for audio files
    Library,
    /// ReplayGain analysis
    ReplayGain,
    /// Bliss audio analysis for similarity
    Bliss,
    /// Waveform generation
    Waveform,
}

impl ScanType {
    pub fn title(&self) -> &'static str {
        match self {
            ScanType::Library => "Library Scan",
            ScanType::ReplayGain => "ReplayGain Analysis",
            ScanType::Bliss => "Bliss Audio Analysis",
            ScanType::Waveform => "Waveform Generation",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ScanType::Library => "Scanning directories for audio files...",
            ScanType::ReplayGain => "Analyzing audio levels for normalization...",
            ScanType::Bliss => "Extracting audio features for similarity...",
            ScanType::Waveform => "Generating visual waveforms...",
        }
    }
}

/// State for the scan progress modal
#[derive(Debug, Clone)]
pub struct ScanProgressModal {
    /// Which type of scan is active
    pub scan_type: ScanType,
    /// Whether the modal is visible (can be dismissed to run in background)
    pub visible: bool,
}

impl ScanProgressModal {
    pub fn new(scan_type: ScanType) -> Self {
        Self {
            scan_type,
            visible: true,
        }
    }
}
