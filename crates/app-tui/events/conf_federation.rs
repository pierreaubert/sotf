use super::PlayerCommand;
use crate::app::{
    App, FederationEditState, FederationMode, InputMode, ADD_SOURCE_TYPE_IDX, SOURCE_TYPE_NAMES,
};
use crossterm::event::{KeyCode, KeyEvent};
use sotf_audio_player::federation_config::{FederationSourceEntry, SourceConnectionConfig};

pub(super) fn handle_federation_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match app.federation_state.mode {
        FederationMode::List => handle_list_mode(app, key),
        FederationMode::EditSource => handle_edit_mode(app, key),
        FederationMode::AddSource => handle_add_mode(app, key),
    }
}

fn handle_list_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    let state = &mut app.federation_state;
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Configure;
        }
        KeyCode::Up => {
            if !state.sources.is_empty() && state.selected_idx > 0 {
                state.selected_idx -= 1;
            }
        }
        KeyCode::Down => {
            if state.selected_idx + 1 < state.sources.len() {
                state.selected_idx += 1;
            }
        }
        KeyCode::Char('a') => {
            ADD_SOURCE_TYPE_IDX.store(0, std::sync::atomic::Ordering::Relaxed);
            state.mode = FederationMode::AddSource;
        }
        KeyCode::Char('d') => {
            if let Some(source) = state.sources.get(state.selected_idx) {
                let source_id = source.source_id.clone();
                // Don't allow deleting the local source
                if source_id != "local" {
                    if let Some(db) = app.library.get_database() {
                        let _ = db.delete_federation_source(&source_id);
                    }
                    state.sources.retain(|s| s.source_id != source_id);
                    if state.selected_idx >= state.sources.len() && !state.sources.is_empty() {
                        state.selected_idx = state.sources.len() - 1;
                    }
                }
            }
        }
        KeyCode::Char('e') | KeyCode::Enter => {
            if let Some(source) = state.sources.get(state.selected_idx).cloned()
                && source.source_id != "local"
            {
                state.edit = Some(FederationEditState::new(source, false));
                state.mode = FederationMode::EditSource;
            }
        }
        KeyCode::Char(' ') => {
            if let Some(source) = state.sources.get_mut(state.selected_idx)
                && source.source_id != "local"
            {
                source.is_enabled = !source.is_enabled;
                if let Some(db) = app.library.get_database() {
                    let _ = db.toggle_federation_source(&source.source_id);
                }
            }
        }
        _ => {}
    }
    None
}

fn handle_edit_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    let state = &mut app.federation_state;
    let Some(edit) = &mut state.edit else {
        state.mode = FederationMode::List;
        return None;
    };

    if edit.editing_value {
        match key.code {
            KeyCode::Enter => {
                let value = edit.edit_buffer.clone();
                edit.set_field_value(edit.selected_field, &value);
                edit.editing_value = false;
            }
            KeyCode::Esc => {
                edit.editing_value = false;
            }
            KeyCode::Backspace => {
                edit.edit_buffer.pop();
            }
            KeyCode::Char(c) => {
                edit.edit_buffer.push(c);
            }
            _ => {}
        }
        return None;
    }

    match key.code {
        KeyCode::Esc => {
            state.edit = None;
            state.mode = FederationMode::List;
        }
        KeyCode::Up => {
            if edit.selected_field > 0 {
                edit.selected_field -= 1;
            }
        }
        KeyCode::Down => {
            if edit.selected_field + 1 < edit.field_count() {
                edit.selected_field += 1;
            }
        }
        KeyCode::Enter => {
            edit.edit_buffer = edit.field_value(edit.selected_field);
            edit.editing_value = true;
        }
        KeyCode::Char('s') | KeyCode::Tab => {
            // Save
            let source = edit.source.clone();
            let is_new = edit.is_new;
            state.edit = None;
            state.mode = FederationMode::List;

            if let Some(db) = app.library.get_database() {
                let _ = db.save_federation_source(&source);
            }
            if is_new {
                state.sources.push(source);
            } else {
                for s in &mut state.sources {
                    if s.source_id == source.source_id {
                        *s = source.clone();
                        break;
                    }
                }
            }
        }
        _ => {}
    }
    None
}

fn handle_add_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    let state = &mut app.federation_state;
    let idx = ADD_SOURCE_TYPE_IDX.load(std::sync::atomic::Ordering::Relaxed);

    match key.code {
        KeyCode::Esc => {
            state.mode = FederationMode::List;
        }
        KeyCode::Up => {
            if idx > 0 {
                ADD_SOURCE_TYPE_IDX.store(idx - 1, std::sync::atomic::Ordering::Relaxed);
            }
        }
        KeyCode::Down => {
            if idx + 1 < SOURCE_TYPE_NAMES.len() {
                ADD_SOURCE_TYPE_IDX.store(idx + 1, std::sync::atomic::Ordering::Relaxed);
            }
        }
        KeyCode::Enter => {
            let type_name = SOURCE_TYPE_NAMES[idx].0;
            let display_name = SOURCE_TYPE_NAMES[idx].1.to_string();
            let connection = SourceConnectionConfig::default_for_type(type_name);
            let source_id = format!("{}:{}", type_name, uuid_short());
            let source = FederationSourceEntry {
                source_id,
                display_name,
                priority: 50,
                is_enabled: true,
                connection,
            };
            state.edit = Some(FederationEditState::new(source, true));
            state.mode = FederationMode::EditSource;
        }
        _ => {}
    }
    None
}

/// Generate a short unique-ish identifier
fn uuid_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    format!("{:x}", ts & 0xFFFF_FFFF)
}
