use crate::app::{App, PlaylistMode};
use crate::ui::keybinding_catalog::{PlaylistCommand, TuiCommand, TuiKeyContext, resolve_command};
use crossterm::event::{KeyCode, KeyEvent};

use super::PlayerCommand;

pub fn handle_playlists_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match app.playlists.mode {
        PlaylistMode::List => handle_list_mode(app, key),
        PlaylistMode::Tracks => handle_tracks_mode(app, key),
        PlaylistMode::Create => handle_text_input(app, key, true),
        PlaylistMode::Rename => handle_text_input(app, key, false),
        PlaylistMode::ConfirmDelete => handle_confirm_delete(app, key),
    }
}

fn handle_list_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    let command = match resolve_command(TuiKeyContext::PlaylistList, key) {
        Some(TuiCommand::Playlist(command)) => command,
        Some(command) => unreachable!("non-playlist command in PlaylistList: {command:?}"),
        None => return None,
    };

    match command {
        PlaylistCommand::Navigate => {
            if matches!(key.code, KeyCode::Up | KeyCode::Char('k')) {
                app.playlists.controller.select_prev_playlist();
            } else {
                app.playlists.controller.select_next_playlist();
            }
            None
        }
        PlaylistCommand::Open => {
            if let Some(db) = app.library.get_database() {
                let idx = app.playlists.controller.selected_playlist_index;
                match app.playlists.controller.open_playlist(db, idx) {
                    Ok(()) => app.playlists.mode = PlaylistMode::Tracks,
                    Err(e) => app.ui.status_message = Some(format!("Error: {}", e)),
                }
            }
            None
        }
        PlaylistCommand::Create => {
            app.playlists.name_input.clear();
            app.playlists.mode = PlaylistMode::Create;
            None
        }
        PlaylistCommand::Rename => {
            if let Some(playlist) = app
                .playlists
                .controller
                .playlists()
                .get(app.playlists.controller.selected_playlist_index)
            {
                app.playlists.name_input = playlist.name.clone();
                app.playlists.mode = PlaylistMode::Rename;
            }
            None
        }
        PlaylistCommand::Delete => {
            if !app.playlists.controller.playlists().is_empty() {
                app.playlists.mode = PlaylistMode::ConfirmDelete;
            }
            None
        }
        PlaylistCommand::PlayAll => play_selected_playlist(app),
        PlaylistCommand::Import => {
            use crate::app::{FilePickerMode, FilePickerOrigin};
            app.open_file_explorer(
                FilePickerOrigin::PlaylistImport,
                FilePickerMode::File,
                "Import M3U Playlist",
                None,
                Some("m3u,m3u8"),
            );
            None
        }
        PlaylistCommand::Export => {
            use crate::app::{FilePickerMode, FilePickerOrigin};
            let has_active = app.playlists.controller.active_playlist().is_some();
            if !has_active && let Some(db) = app.library.get_database() {
                let idx = app.playlists.controller.selected_playlist_index;
                let _ = app.playlists.controller.open_playlist(db, idx);
            }
            if app.playlists.controller.active_playlist().is_some() {
                app.open_file_explorer(
                    FilePickerOrigin::PlaylistExport,
                    FilePickerMode::Directory,
                    "Export Playlist (select directory)",
                    None,
                    None,
                );
            } else {
                app.ui.status_message = Some("No playlist to export".to_string());
            }
            None
        }
        PlaylistCommand::Back | PlaylistCommand::RemoveTrack | PlaylistCommand::MoveTrack => {
            unreachable!("tracks-only command resolved in PlaylistList: {command:?}")
        }
    }
}

fn handle_tracks_mode(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    let command = match resolve_command(TuiKeyContext::PlaylistTracks, key) {
        Some(TuiCommand::Playlist(command)) => command,
        Some(command) => unreachable!("non-playlist command in PlaylistTracks: {command:?}"),
        None => return None,
    };

    match command {
        PlaylistCommand::Navigate => {
            if matches!(key.code, KeyCode::Up | KeyCode::Char('k')) {
                app.playlists.controller.select_prev_track();
            } else {
                app.playlists.controller.select_next_track();
            }
            None
        }
        PlaylistCommand::Back => {
            app.playlists.controller.close_playlist();
            app.playlists.mode = PlaylistMode::List;
            None
        }
        PlaylistCommand::RemoveTrack => {
            if let Some(db) = app.library.get_database() {
                let idx = app.playlists.controller.selected_track_index;
                if let Err(e) = app.playlists.controller.remove_track(db, idx) {
                    app.ui.status_message = Some(format!("Error: {}", e));
                }
            }
            None
        }
        PlaylistCommand::MoveTrack => {
            if let Some(db) = app.library.get_database() {
                let result = if key.code == KeyCode::Char('K') {
                    app.playlists.controller.move_track_up(db)
                } else {
                    app.playlists.controller.move_track_down(db)
                };
                if let Err(error) = result {
                    app.ui.status_message = Some(format!("Error: {error}"));
                }
            }
            None
        }
        PlaylistCommand::PlayAll => play_active_playlist(app),
        PlaylistCommand::Open
        | PlaylistCommand::Create
        | PlaylistCommand::Rename
        | PlaylistCommand::Delete
        | PlaylistCommand::Import
        | PlaylistCommand::Export => {
            unreachable!("list-only command resolved in PlaylistTracks: {command:?}")
        }
    }
}

fn handle_text_input(app: &mut App, key: KeyEvent, is_create: bool) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Char(c) => {
            app.playlists.name_input.push(c);
            None
        }
        KeyCode::Backspace => {
            app.playlists.name_input.pop();
            None
        }
        KeyCode::Enter => {
            let name = app.playlists.name_input.trim().to_string();
            if !name.is_empty()
                && let Some(db) = app.library.get_database()
            {
                if is_create {
                    match app.playlists.controller.create_playlist(db, &name, None) {
                        Ok(_) => app.ui.status_message = Some(format!("Created '{}'", name)),
                        Err(e) => app.ui.status_message = Some(format!("Error: {}", e)),
                    }
                } else {
                    let idx = app.playlists.controller.selected_playlist_index;
                    match app.playlists.controller.rename_playlist(db, idx, &name) {
                        Ok(()) => app.ui.status_message = Some(format!("Renamed to '{}'", name)),
                        Err(e) => app.ui.status_message = Some(format!("Error: {}", e)),
                    }
                }
            }
            app.playlists.name_input.clear();
            app.playlists.mode = PlaylistMode::List;
            None
        }
        KeyCode::Esc => {
            app.playlists.name_input.clear();
            app.playlists.mode = PlaylistMode::List;
            None
        }
        _ => None,
    }
}

fn handle_confirm_delete(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => {
            if let Some(db) = app.library.get_database() {
                let idx = app.playlists.controller.selected_playlist_index;
                match app.playlists.controller.delete_playlist(db, idx) {
                    Ok(()) => app.ui.status_message = Some("Playlist deleted".to_string()),
                    Err(e) => app.ui.status_message = Some(format!("Error: {}", e)),
                }
            }
            app.playlists.mode = PlaylistMode::List;
            None
        }
        _ => {
            // Any other key cancels
            app.playlists.mode = PlaylistMode::List;
            None
        }
    }
}

/// Play all tracks from the currently selected playlist.
fn play_selected_playlist(app: &mut App) -> Option<PlayerCommand> {
    if let Some(db) = app.library.get_database() {
        let idx = app.playlists.controller.selected_playlist_index;
        if app.playlists.controller.open_playlist(db, idx).is_ok() {
            return play_active_playlist(app);
        }
    }
    None
}

/// Play all tracks from the active (open) playlist by adding them to queue.
///
/// Idempotent on the track-path set: a track already present in the queue is
/// skipped rather than appended. Without this, repeated `p` presses (or just
/// pressing `p` once in List mode and again in Tracks mode) would clone the
/// playlist into the queue every time.
fn play_active_playlist(app: &mut App) -> Option<PlayerCommand> {
    use crate::app::{QueueEntry, QueueItem};

    let track_paths = app.playlists.controller.active_track_paths();
    if track_paths.is_empty() {
        app.ui.status_message = Some("Playlist is empty".to_string());
        return None;
    }

    let was_empty = app.queue.is_empty();
    let mut added = 0usize;
    let mut skipped = 0usize;

    for path in &track_paths {
        // Skip if this track is already represented by an existing queue entry.
        let already_queued = app
            .queue
            .iter()
            .any(|e| e.item.album.tracks.iter().any(|t| &t.path == path));
        if already_queued {
            skipped += 1;
            continue;
        }

        // Find the album that owns this track and push a single-track copy.
        for album in &app.library.albums {
            if album.tracks.iter().any(|t| &t.path == path) {
                let mut single = album.clone();
                single.tracks.retain(|t| &t.path == path);
                if !single.tracks.is_empty() {
                    app.queue.push(QueueEntry::new(QueueItem::new(single)));
                    added += 1;
                }
                break;
            }
        }
    }

    if added == 0 && skipped > 0 {
        app.ui.status_message = Some(format!(
            "Playlist already in queue ({} track{})",
            skipped,
            if skipped == 1 { "" } else { "s" }
        ));
    }

    // Auto-play if the queue was previously empty AND we added something.
    if was_empty && added > 0 {
        return app.start_queue().map(PlayerCommand::Play);
    }
    None
}
