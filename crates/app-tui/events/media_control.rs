//! Media control event handling (system media keys, etc.)

use super::PlayerCommand;
use crate::app::App;

/// Handle media control events from the system (play/pause/next/etc.)
pub fn handle_media_control_event(
    app: &mut App,
    event: souvlaki::MediaControlEvent,
) -> Option<PlayerCommand> {
    match event {
        souvlaki::MediaControlEvent::Play => {
            if app.current_queue_index.is_none() {
                // Nothing playing yet — start the queue
                app.start_queue().map(PlayerCommand::Play)
            } else {
                app.is_playing = true;
                Some(PlayerCommand::Resume)
            }
        }
        souvlaki::MediaControlEvent::Pause => {
            app.is_playing = false;
            Some(PlayerCommand::Pause)
        }
        souvlaki::MediaControlEvent::Toggle => {
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
        souvlaki::MediaControlEvent::Next => {
            if let Some(path) = app.next_track() {
                Some(PlayerCommand::Play(path))
            } else {
                app.is_playing = false;
                Some(PlayerCommand::Stop)
            }
        }
        souvlaki::MediaControlEvent::Previous => app.previous_track().map(PlayerCommand::Play),
        souvlaki::MediaControlEvent::Stop => {
            app.is_playing = false;
            Some(PlayerCommand::Stop)
        }
        souvlaki::MediaControlEvent::SetPosition(pos) => {
            Some(PlayerCommand::Seek(pos.0.as_secs_f64()))
        }
        souvlaki::MediaControlEvent::SetVolume(vol) => {
            let clamped = vol.clamp(0.0, 1.0) as f32;
            app.volume = clamped;
            Some(PlayerCommand::SetVolume(clamped))
        }
        souvlaki::MediaControlEvent::Seek(direction) => {
            let offset = match direction {
                souvlaki::SeekDirection::Forward => 10.0,
                souvlaki::SeekDirection::Backward => -10.0,
            };
            Some(PlayerCommand::SeekRelative(offset))
        }
        souvlaki::MediaControlEvent::SeekBy(direction, duration) => {
            let secs = duration.as_secs_f64();
            let offset = match direction {
                souvlaki::SeekDirection::Forward => secs,
                souvlaki::SeekDirection::Backward => -secs,
            };
            Some(PlayerCommand::SeekRelative(offset))
        }
        souvlaki::MediaControlEvent::Raise => None,
        souvlaki::MediaControlEvent::Quit => {
            app.should_quit = true;
            None
        }
        souvlaki::MediaControlEvent::OpenUri(_) => None,
    }
}
