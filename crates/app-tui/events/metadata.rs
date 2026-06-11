use crate::app::{App, InputMode, MetadataEditorState};
use crate::events::PlayerCommand;
use crossterm::event::{KeyCode, KeyEvent};

pub(super) fn handle_metadata_editor_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    if app
        .metadata_editor
        .as_ref()
        .is_some_and(|editor| editor.editing)
    {
        return handle_field_edit(app, key);
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.metadata_editor = None;
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Up => {
            if let Some(editor) = &mut app.metadata_editor {
                editor.selected_field = editor.selected_field.saturating_sub(1);
            }
        }
        KeyCode::Down => {
            if let Some(editor) = &mut app.metadata_editor {
                editor.selected_field =
                    (editor.selected_field + 1).min(MetadataEditorState::FIELD_COUNT - 1);
            }
        }
        KeyCode::Left => {
            if let Some(editor) = &mut app.metadata_editor {
                editor.selected_result = editor.selected_result.saturating_sub(1);
            }
        }
        KeyCode::Right => {
            if let Some(editor) = &mut app.metadata_editor
                && !editor.search_results.is_empty()
            {
                editor.selected_result =
                    (editor.selected_result + 1).min(editor.search_results.len() - 1);
            }
        }
        KeyCode::Enter => {
            if let Some(editor) = &mut app.metadata_editor {
                editor.editing = true;
                editor.edit_buffer = editor.field_value(editor.selected_field).to_string();
            }
        }
        KeyCode::Char('p') => refresh_metadata_preview(app),
        KeyCode::Char('s') => apply_metadata_editor(app),
        KeyCode::Char('b') => search_musicbrainz(app),
        KeyCode::Char('i') => import_selected_candidate(app),
        _ => {}
    }
    None
}

fn handle_field_edit(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Esc => {
            if let Some(editor) = &mut app.metadata_editor {
                editor.editing = false;
                editor.edit_buffer.clear();
            }
        }
        KeyCode::Enter => {
            if let Some(editor) = &mut app.metadata_editor {
                let value = std::mem::take(&mut editor.edit_buffer);
                editor.set_field_value(editor.selected_field, value);
                editor.editing = false;
            }
            refresh_metadata_preview(app);
        }
        KeyCode::Backspace => {
            if let Some(editor) = &mut app.metadata_editor {
                editor.edit_buffer.pop();
            }
        }
        KeyCode::Char(ch) => {
            if let Some(editor) = &mut app.metadata_editor {
                editor.edit_buffer.push(ch);
            }
        }
        _ => {}
    }
    None
}

fn refresh_metadata_preview(app: &mut App) {
    let Some(editor) = app.metadata_editor.clone() else {
        return;
    };
    let result = editor.patch().and_then(|patch| {
        sotf_audio_player::MetadataController::preview_edit(
            &app.library,
            editor.target.clone(),
            patch,
        )
        .map_err(|err| err.to_string())
    });

    if let Some(current) = &mut app.metadata_editor {
        match result {
            Ok(preview) => {
                current.preview = Some(preview);
                current.error = None;
            }
            Err(err) => {
                current.preview = None;
                current.error = Some(err);
            }
        }
    }
}

fn apply_metadata_editor(app: &mut App) {
    let Some(editor) = app.metadata_editor.clone() else {
        return;
    };
    let result = editor.patch().and_then(|patch| {
        sotf_audio_player::MetadataController::apply_edit(
            &mut app.library,
            editor.target.clone(),
            patch,
        )
        .map_err(|err| err.to_string())
    });

    match result {
        Ok(preview) => {
            app.metadata_editor = None;
            app.input_mode = InputMode::Normal;
            app.needs_filter_update = true;
            app.status_message = Some(format!(
                "Metadata updated for {} file(s)",
                preview.affected_files.len()
            ));
        }
        Err(err) => {
            if let Some(current) = &mut app.metadata_editor {
                current.error = Some(err);
            }
        }
    }
}

fn search_musicbrainz(app: &mut App) {
    let Some(editor) = app.metadata_editor.clone() else {
        return;
    };
    let query = editor.search_query.trim().to_string();
    if query.is_empty() {
        if let Some(current) = &mut app.metadata_editor {
            current.search_error = Some("Enter a MusicBrainz search query".to_string());
        }
        return;
    }

    let result = run_musicbrainz_search(editor.scope, query);
    if let Some(current) = &mut app.metadata_editor {
        match result {
            Ok(candidates) => {
                current.search_results = candidates;
                current.selected_result = 0;
                current.search_error = None;
            }
            Err(err) => {
                current.search_results.clear();
                current.search_error = Some(err);
            }
        }
    }
}

fn run_musicbrainz_search(
    scope: crate::app::MetadataEditorScope,
    query: String,
) -> Result<Vec<sotf_audio_player::MetadataImportCandidate>, String> {
    use sotf_audio_player::metadata::MetadataProvider;

    let config = sotf_audio_player::config::load_metadata_services_config()
        .unwrap_or_else(|_| sotf_audio_player::MetadataServicesConfig::default());
    let provider_config = config
        .providers
        .iter()
        .find(|provider| provider.provider_id == "musicbrainz")
        .cloned()
        .unwrap_or_default();
    if !provider_config.enabled {
        return Err("MusicBrainz is disabled in Metadata settings".to_string());
    }

    let provider = sotf_audio_player::MusicBrainzProvider::with_endpoint(
        provider_config.endpoint,
        config.user_agent,
    )
    .map_err(|err| err.to_string())?;
    let runtime = tokio::runtime::Runtime::new().map_err(|err| err.to_string())?;
    runtime
        .block_on(async {
            match scope {
                crate::app::MetadataEditorScope::Album => provider.search_album(None, &query).await,
                crate::app::MetadataEditorScope::Track => provider.search_track(None, &query).await,
            }
        })
        .map_err(|err| err.to_string())
}

fn import_selected_candidate(app: &mut App) {
    if let Some(editor) = &mut app.metadata_editor
        && let Some(candidate) = editor.search_results.get(editor.selected_result).cloned()
    {
        editor.apply_candidate(candidate);
    }
    refresh_metadata_preview(app);
}
