use super::PlayerCommand;
use crate::app::App;
use crate::ui::keybinding_catalog::{QueueCommand, TuiCommand, TuiKeyContext, resolve_command};
use crossterm::event::{KeyCode, KeyEvent};

pub(super) fn handle_queue_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match resolve_command(TuiKeyContext::Queue, key) {
        Some(TuiCommand::Queue(command)) => handle_documented_command(app, key, command),
        Some(command) => unreachable!("non-queue command in Queue context: {command:?}"),
        None => handle_undocumented_command(app, key),
    }
}

fn handle_documented_command(
    app: &mut App,
    key: KeyEvent,
    command: QueueCommand,
) -> Option<PlayerCommand> {
    match command {
        QueueCommand::Navigate => {
            if matches!(key.code, KeyCode::Up | KeyCode::Char('k')) {
                app.select_previous_queue_item();
            } else {
                app.select_next_queue_item();
            }
            None
        }
        QueueCommand::PlaySelected => app.jump_to_selected_album().map(PlayerCommand::Play),
        QueueCommand::ToggleExpanded => {
            if matches!(key.code, KeyCode::Right | KeyCode::Char('l')) {
                app.expand_queue_item();
            } else {
                app.collapse_queue_item();
            }
            None
        }
        QueueCommand::PlayPause => {
            if app.playback.current_queue_index.is_none() {
                return app.start_queue().map(PlayerCommand::Play);
            }
            if app.playback.is_playing {
                app.playback.is_playing = false;
                Some(PlayerCommand::Pause)
            } else {
                app.playback.is_playing = true;
                Some(PlayerCommand::Resume)
            }
        }
        QueueCommand::NextTrack => {
            if let Some(path) = app.next_track() {
                Some(PlayerCommand::Play(path))
            } else {
                app.playback.is_playing = false;
                Some(PlayerCommand::Stop)
            }
        }
        QueueCommand::PreviousTrack => app.previous_track().map(PlayerCommand::Play),
        QueueCommand::Remove => match app.remove_from_queue(app.queue_view.selected_index) {
            sotf_audio_player::QueuePlaybackEffect::Reload(path)
            | sotf_audio_player::QueuePlaybackEffect::Play(path) => Some(PlayerCommand::Play(path)),
            sotf_audio_player::QueuePlaybackEffect::Stop => Some(PlayerCommand::Stop),
            sotf_audio_player::QueuePlaybackEffect::None => None,
        },
        QueueCommand::Clear => {
            app.clear_queue();
            Some(PlayerCommand::Stop)
        }
        QueueCommand::AddToPlaylist => add_selection_to_active_playlist(app),
    }
}

fn handle_undocumented_command(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        #[cfg(not(target_os = "windows"))]
        KeyCode::Char('[') => {
            app.prev_album_image();
            None
        }
        #[cfg(not(target_os = "windows"))]
        KeyCode::Char(']') => {
            app.next_album_image();
            None
        }
        KeyCode::Char('.') => Some(PlayerCommand::SeekRelative(10.0)),
        KeyCode::Char(',') => Some(PlayerCommand::SeekRelative(-10.0)),
        KeyCode::Char(':') => Some(PlayerCommand::SeekRelative(30.0)),
        KeyCode::Char(';') => Some(PlayerCommand::SeekRelative(-30.0)),
        KeyCode::Char('f') => {
            app.toggle_current_queue_album_favorite();
            None
        }
        KeyCode::Char('m') => {
            let entry = app.queue.get(app.queue_view.selected_index)?;
            let target_track = app
                .queue_view
                .selected_track_index
                .and_then(|index| entry.item.album.tracks.get(index))
                .or_else(|| entry.item.current_track())
                .or_else(|| entry.item.album.tracks.first());
            if let Some(track) = target_track {
                app.modal.metadata_editor = Some(crate::app::MetadataEditorState::for_track(track));
                app.input_mode = crate::app::InputMode::MetadataEditor;
            }
            None
        }
        _ => None,
    }
}

fn add_selection_to_active_playlist(app: &mut App) -> Option<PlayerCommand> {
    if app.playlists.controller.active_playlist_id().is_none() {
        app.ui.status_message = Some("Open a playlist first (Y screen)".to_string());
        return None;
    }
    let database = app.library.get_database()?;
    let entry = app.queue.get(app.queue_view.selected_index)?;

    if let Some(track_index) = app.queue_view.selected_track_index
        && let Some(track) = entry.item.album.tracks.get(track_index)
    {
        let title = track
            .title
            .clone()
            .unwrap_or_else(|| format!("Track {}", track_index + 1));
        match app
            .playlists
            .controller
            .add_tracks(database, std::slice::from_ref(&track.path))
        {
            Ok(()) => app.ui.status_message = Some(format!("Added '{title}' to playlist")),
            Err(error) => app.ui.status_message = Some(format!("Error: {error}")),
        }
    } else {
        let playlist_index = app
            .playlists
            .controller
            .active_playlist_id()
            .and_then(|id| {
                app.playlists
                    .controller
                    .playlists()
                    .iter()
                    .position(|playlist| playlist.id == Some(id))
            });
        if let Some(playlist_index) = playlist_index {
            let album = entry.item.album.clone();
            match app
                .playlists
                .controller
                .add_album_to_playlist(database, playlist_index, &album)
            {
                Ok(()) => {
                    app.ui.status_message = Some(format!("Added '{}' to playlist", album.title))
                }
                Err(error) => app.ui.status_message = Some(format!("Error: {error}")),
            }
        }
    }
    None
}
