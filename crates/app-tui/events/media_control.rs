//! Media control event handling (system media keys, etc.)

use super::PlayerCommand;
use crate::app::App;

/// Handle media control events from the system (play/pause/next/etc.)
pub fn handle_media_control_event(
    app: &mut App,
    event: sotf_media_controls::MediaControlEvent,
) -> Option<PlayerCommand> {
    match event {
        sotf_media_controls::MediaControlEvent::Play => {
            if app.current_queue_index.is_none() {
                // Nothing playing yet — start the queue
                app.start_queue().map(PlayerCommand::Play)
            } else {
                app.is_playing = true;
                Some(PlayerCommand::Resume)
            }
        }
        sotf_media_controls::MediaControlEvent::Pause => {
            app.is_playing = false;
            Some(PlayerCommand::Pause)
        }
        sotf_media_controls::MediaControlEvent::Toggle => {
            if app.is_playing {
                app.is_playing = false;
                Some(PlayerCommand::Pause)
            } else if app.current_queue_index.is_none() {
                app.start_queue().map(PlayerCommand::Play)
            } else {
                app.is_playing = true;
                Some(PlayerCommand::Resume)
            }
        }
        sotf_media_controls::MediaControlEvent::Next => {
            if let Some(path) = app.next_track() {
                Some(PlayerCommand::Play(path))
            } else {
                app.is_playing = false;
                Some(PlayerCommand::Stop)
            }
        }
        sotf_media_controls::MediaControlEvent::Previous => {
            app.previous_track().map(PlayerCommand::Play)
        }
        sotf_media_controls::MediaControlEvent::Stop => {
            app.is_playing = false;
            Some(PlayerCommand::Stop)
        }
        sotf_media_controls::MediaControlEvent::SetPosition(pos) => {
            Some(PlayerCommand::Seek(pos.0.as_secs_f64()))
        }
        sotf_media_controls::MediaControlEvent::SetVolume(vol) => {
            let clamped = vol.clamp(0.0, 1.0) as f32;
            app.volume = clamped;
            Some(PlayerCommand::SetVolume(clamped))
        }
        sotf_media_controls::MediaControlEvent::Seek(direction) => {
            let offset = match direction {
                sotf_media_controls::SeekDirection::Forward => 10.0,
                sotf_media_controls::SeekDirection::Backward => -10.0,
            };
            Some(PlayerCommand::SeekRelative(offset))
        }
        sotf_media_controls::MediaControlEvent::SeekBy(direction, duration) => {
            let secs = duration.as_secs_f64();
            let offset = match direction {
                sotf_media_controls::SeekDirection::Forward => secs,
                sotf_media_controls::SeekDirection::Backward => -secs,
            };
            Some(PlayerCommand::SeekRelative(offset))
        }
        sotf_media_controls::MediaControlEvent::Raise => None,
        sotf_media_controls::MediaControlEvent::Quit => {
            app.should_quit = true;
            None
        }
        sotf_media_controls::MediaControlEvent::OpenUri(_) => None,
    }
}
