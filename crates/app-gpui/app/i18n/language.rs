use serde::{Deserialize, Serialize};

/// Available language identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    #[default]
    English,
    French,
    German,
    Spanish,
    /// QA-only expansion locale. Deliberately excluded from `all()`.
    Pseudo,
}

impl Language {
    pub fn all() -> &'static [Language] {
        &[
            Language::English,
            Language::French,
            Language::German,
            Language::Spanish,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::French => "Français",
            Language::German => "Deutsch",
            Language::Spanish => "Español",
            Language::Pseudo => "Pseudo (QA)",
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::French => "fr",
            Language::German => "de",
            Language::Spanish => "es",
            Language::Pseudo => "qps-ploc",
        }
    }

    pub fn next(&self) -> Language {
        match self {
            Language::English => Language::French,
            Language::French => Language::German,
            Language::German => Language::Spanish,
            Language::Spanish => Language::English,
            Language::Pseudo => Language::English,
        }
    }

    pub fn is_pseudo(self) -> bool {
        self == Language::Pseudo
    }

    pub fn base(self) -> Language {
        if self.is_pseudo() {
            Language::English
        } else {
            self
        }
    }
}
