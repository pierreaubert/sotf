use super::auto_eq_translations::AutoEqTranslations;
use super::types::DocKey;
use gpui::Global;
use gpui_ui_kit::i18n::Language;

/// Global state for `AutoEQ` i18n
#[derive(Debug)]
pub struct AutoEqI18nState {
    pub language: Language,
    pub translations: AutoEqTranslations,
}

impl Global for AutoEqI18nState {}

impl AutoEqI18nState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            language: Language::default(),
            translations: AutoEqTranslations::new(),
        }
    }

    #[must_use]
    pub fn t(&self, key: DocKey) -> &'static str {
        self.translations.get(self.language, key)
    }
}

impl Default for AutoEqI18nState {
    fn default() -> Self {
        Self::new()
    }
}
