use gpui::SharedString;
use gpui_design::DesignLanguage;

pub(super) fn design_language_value(language: Option<&str>) -> &'static str {
    language
        .and_then(DesignLanguage::from_id)
        .map_or("system", |language| language.as_str())
}

#[expect(
    clippy::option_option,
    reason = "The outer option distinguishes an uninitialized or unhandled value from an initialized absent value"
)]
pub(super) fn design_language_from_value(value: &SharedString) -> Option<Option<DesignLanguage>> {
    if value.as_ref() == "system" {
        Some(None)
    } else {
        DesignLanguage::from_id(value.as_ref()).map(Some)
    }
}
