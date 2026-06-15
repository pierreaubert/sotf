use super::super::tests::make_app;
use crate::app::{ConfigureSubScreen, InputMode, Screen, SpinoramaStep};

/// Create an app already on the Library screen (past Loading).
pub(super) fn app_on_library() -> crate::app::App {
    let mut app = make_app();
    app.current_screen = Screen::Library;
    app
}

/// Create an app on Configure > SpinoramaEq with speakers loaded,
/// ready for wizard navigation tests.
pub(super) fn app_on_spinorama_select() -> crate::app::App {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::SpinoramaEq;
    app.input_mode = InputMode::ConfigureSpinoramaEq;
    app.spinorama_eq.step = SpinoramaStep::Select;
    // Pre-populate speaker list so Enter can select one
    app.spinorama_eq.model.available_speakers = vec![
        "Speaker A".to_string(),
        "Speaker B".to_string(),
        "Speaker C".to_string(),
    ];
    app.spinorama_eq.update_filter();
    app
}

/// Create an app on Configure > RoomEq, tab content focused.
pub(super) fn app_on_room_eq() -> crate::app::App {
    let mut app = app_on_library();
    app.current_screen = Screen::Configure;
    app.configure_sub_screen = ConfigureSubScreen::RoomEq;
    app.input_mode = InputMode::ConfigureRoomEq;
    app.room_eq.step_tab_focused = true;
    app
}
