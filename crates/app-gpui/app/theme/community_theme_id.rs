use super::Theme;
use gpui::SharedString;
use gpui_themes::{
    BuiltInThemePreset, CommunityThemeBundle, CommunityThemeManifest, EditorTheme,
    ThemeModePreference,
};
use serde::{Deserialize, Serialize};

/// Curated community themes exposed by the app gallery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommunityThemeId {
    Nord,
    Dracula,
}

impl CommunityThemeId {
    #[must_use]
    pub fn all() -> &'static [CommunityThemeId] {
        &[CommunityThemeId::Nord, CommunityThemeId::Dracula]
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            CommunityThemeId::Nord => "Nord",
            CommunityThemeId::Dracula => "Dracula",
        }
    }

    #[must_use]
    pub fn value(self) -> &'static str {
        match self {
            CommunityThemeId::Nord => "nord",
            CommunityThemeId::Dracula => "dracula",
        }
    }

    #[must_use]
    pub fn author(self) -> &'static str {
        match self {
            CommunityThemeId::Nord | CommunityThemeId::Dracula => "SOTF Community",
        }
    }

    #[must_use]
    pub fn tags(self) -> &'static [&'static str] {
        match self {
            CommunityThemeId::Nord => &["community", "dark", "terminal"],
            CommunityThemeId::Dracula => &["community", "dark", "base16"],
        }
    }

    #[must_use]
    pub fn from_value(value: &SharedString) -> Option<Self> {
        Self::from_id(value.as_ref())
    }

    #[must_use]
    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "nord" => Some(Self::Nord),
            "dracula" => Some(Self::Dracula),
            _ => None,
        }
    }

    #[must_use]
    pub fn built_in_preset(self) -> BuiltInThemePreset {
        match self {
            CommunityThemeId::Nord => BuiltInThemePreset::Nord,
            CommunityThemeId::Dracula => BuiltInThemePreset::Dracula,
        }
    }

    #[must_use]
    pub fn editor_theme(self) -> EditorTheme {
        EditorTheme::preset(self.built_in_preset())
    }

    #[must_use]
    pub fn manifest(self) -> CommunityThemeManifest {
        let editor_theme = self.editor_theme();
        let mut manifest = CommunityThemeManifest::for_theme(&editor_theme);
        manifest.author = self.author().to_string();
        manifest.license = "MIT".to_string();
        manifest.tags = self.tags().iter().map(|tag| (*tag).to_string()).collect();
        manifest.preferred_mode = ThemeModePreference::Dark;
        manifest
    }

    #[must_use]
    pub fn bundle(self) -> CommunityThemeBundle {
        CommunityThemeBundle::new(self.manifest(), self.editor_theme())
    }

    #[must_use]
    pub fn theme(self) -> Theme {
        Theme::from_editor_theme(&self.editor_theme())
    }

    /// Serializes community json.
    ///
    /// # Errors
    /// Returns an error if the theme cannot be serialized to the community format.
    pub fn to_community_json(self) -> Result<String, serde_json::Error> {
        self.bundle().to_json()
    }
}
