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
        Self::visible_tabs_for_form_factor(is_ios, false)
    }

    /// Tabs offered on an iPhone-class window.
    ///
    /// On top of the iOS exclusions, phone-class windows drop the
    /// Servers tab: that tab turns the device itself into a media
    /// server (SOTF API, MPD, DLNA), which iPhone users are not
    /// expected to do — they stream from a SotF server (Federation
    /// tab) or a streaming service. iPad-class windows keep it.
    pub fn phone_visible_tabs() -> Vec<SettingsTab> {
        Self::visible_tabs_for_form_factor(cfg!(target_os = "ios"), true)
    }

    pub fn visible_tabs_for_form_factor(is_ios: bool, is_phone: bool) -> Vec<SettingsTab> {
        Self::ALL
            .into_iter()
            .filter(|tab| {
                !(is_ios && matches!(tab, SettingsTab::Library | SettingsTab::Keybindings))
                    && !(is_phone && matches!(tab, SettingsTab::Servers))
            })
            .collect()
    }

    pub fn fallback_for_platform() -> SettingsTab {
        Self::fallback_for_form_factor(false)
    }

    pub fn fallback_for_form_factor(is_phone: bool) -> SettingsTab {
        if is_phone {
            // Servers is hidden on phones; land on the remote-source
            // (stream-from) side of Connections instead.
            SettingsTab::Federation
        } else {
            SettingsTab::Servers
        }
    }
}
