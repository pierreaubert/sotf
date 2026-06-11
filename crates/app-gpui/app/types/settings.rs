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
    Metadata,
    ReleaseChannel,
}

impl SettingsTab {
    pub const ALL: [SettingsTab; 10] = [
        SettingsTab::Library,
        SettingsTab::Theme,
        SettingsTab::Language,
        SettingsTab::Keybindings,
        SettingsTab::AudioDevice,
        SettingsTab::Misc,
        SettingsTab::Federation,
        SettingsTab::Servers,
        SettingsTab::Metadata,
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
