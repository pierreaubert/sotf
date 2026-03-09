use super::PlayerCommand;
use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent};

pub(super) fn handle_queue_keys(app: &mut App, key: KeyEvent) -> Option<PlayerCommand> {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_previous_queue_item();
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next_queue_item();
            None
        }
        KeyCode::Enter => {
            // Jump to selected album and play its first track
            app.jump_to_selected_album().map(PlayerCommand::Play)
        }
        KeyCode::Right | KeyCode::Char('l') => {
            // Expand the selected queue item
            app.expand_queue_item();
            None
        }
        KeyCode::Left | KeyCode::Char('h') => {
            // Collapse the selected queue item (or move to album header if on a track)
            app.collapse_queue_item();
            None
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            app.remove_from_queue(app.selected_queue_index);
            None
        }
        KeyCode::Char('c') => {
            app.clear_queue();
            Some(PlayerCommand::Stop)
        }
        KeyCode::Char('p') => {
            // Play from start or current position
            if app.current_queue_index.is_none() {
                if let Some(path) = app.start_queue() {
                    return Some(PlayerCommand::Play(path));
                }
            } else {
                app.is_playing = true;
                return Some(PlayerCommand::Resume);
            }
            None
        }
        KeyCode::Char(' ') => {
            // Toggle pause
            if app.is_playing {
                app.is_playing = false;
                Some(PlayerCommand::Pause)
            } else {
                app.is_playing = true;
                Some(PlayerCommand::Resume)
            }
        }
        KeyCode::Char('n') | KeyCode::Char('>') => {
            // Next track
            if let Some(path) = app.next_track() {
                Some(PlayerCommand::Play(path))
            } else {
                app.is_playing = false;
                Some(PlayerCommand::Stop)
            }
        }
        KeyCode::Char('b') | KeyCode::Char('<') => {
            // Previous track
            app.previous_track().map(PlayerCommand::Play)
        }
        #[cfg(not(target_os = "windows"))]
        KeyCode::Char('[') => {
            // Previous album image
            app.prev_album_image();
            None
        }
        #[cfg(not(target_os = "windows"))]
        KeyCode::Char(']') => {
            // Next album image
            app.next_album_image();
            None
        }
        // Seek controls
        KeyCode::Char('.') => {
            // Seek forward 10 seconds
            Some(PlayerCommand::SeekRelative(10.0))
        }
        KeyCode::Char(',') => {
            // Seek backward 10 seconds
            Some(PlayerCommand::SeekRelative(-10.0))
        }
        KeyCode::Char(':') => {
            // Seek forward 30 seconds (Shift + ;)
            Some(PlayerCommand::SeekRelative(30.0))
        }
        KeyCode::Char(';') => {
            // Seek backward 30 seconds
            Some(PlayerCommand::SeekRelative(-30.0))
        }
        KeyCode::Char('f') => {
            // Toggle favorite on current queue album
            app.toggle_current_queue_album_favorite();
            None
        }
        // Note: Volume controls (+/-) are now global (see handle_normal_mode)
        _ => None,
    }
}
