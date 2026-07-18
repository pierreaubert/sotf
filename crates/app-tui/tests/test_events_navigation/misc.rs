use super::super::handle_key_event;
use super::super::tests::key;
use crossterm::event::{KeyCode, KeyModifiers};

/// Send a sequence of plain key presses (no modifiers).
pub(super) fn send_keys(app: &mut crate::app::App, codes: &[KeyCode]) {
    for &code in codes {
        handle_key_event(app, key(code));
    }
}

pub(super) fn send_alt_char(app: &mut crate::app::App, value: char) {
    let mut event = key(KeyCode::Char(value));
    event.modifiers = KeyModifiers::ALT;
    handle_key_event(app, event);
}
