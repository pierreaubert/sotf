use super::types::GuideSection;
use crate::app::i18n::DialogTranslations;
use crate::i18n::Language;

pub(super) struct ScreenGuide {
    pub(super) title: &'static str,
    pub(super) overview: &'static str,
    pub(super) sections: &'static [GuideSection],
}

impl ScreenGuide {
    pub(super) fn for_screen(screen: crate::app::Screen, language: Language) -> Self {
        let text = DialogTranslations::for_language(language);
        Self {
            title: text.screen_name(screen),
            overview: text.screen_overview(screen),
            sections: &[],
        }
    }
}
