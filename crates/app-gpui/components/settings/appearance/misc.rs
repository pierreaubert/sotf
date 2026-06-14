use gpui_themes::{ThemeModePreference, ThemeSchedule};

pub(super) fn schedule_from_preference(preference: &ThemeModePreference) -> ThemeSchedule {
    match preference {
        ThemeModePreference::Scheduled { schedule } => *schedule,
        _ => ThemeSchedule::default(),
    }
}
