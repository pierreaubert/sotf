//! macOS backend.
//!
//! Drives `MPRemoteCommandCenter` (incoming media-key events) and
//! `MPNowPlayingInfoCenter` (now-playing metadata + playback state) through
//! the modern `objc2-media-player` bindings — no `cocoa-rs` dependency.

#![allow(unsafe_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSMutableDictionary, NSNumber, NSString};
use objc2_media_player::{
    MPChangePlaybackPositionCommandEvent, MPMediaItemPropertyAlbumTitle, MPMediaItemPropertyArtist,
    MPMediaItemPropertyPlaybackDuration, MPMediaItemPropertyTitle, MPNowPlayingInfoCenter,
    MPNowPlayingInfoPropertyElapsedPlaybackTime, MPNowPlayingPlaybackState, MPRemoteCommand,
    MPRemoteCommandCenter, MPRemoteCommandEvent, MPRemoteCommandHandlerStatus,
};

use crate::{
    Error, MediaControlEvent, MediaMetadata, MediaPlayback, MediaPosition,
    backend::EventHandler, types::PlatformConfig,
};

/// Shared event-dispatch handle. Cloned into every Objective-C block so the
/// blocks (which the OS holds for the lifetime of the process) can call back
/// into the user closure even after `attach()` swaps it.
type SharedHandler = Arc<Mutex<Option<EventHandler>>>;

/// Concrete metadata-dictionary type expected by `MPNowPlayingInfoCenter`.
type InfoDict = NSMutableDictionary<NSString, AnyObject>;

pub(crate) struct MacosBackend {
    handler: SharedHandler,
    /// Targets returned by `addTargetWithHandler` must outlive the dispatch
    /// they enable. `MPRemoteCommandCenter` is a process-wide singleton, so we
    /// drop these only when the backend itself is dropped.
    _targets: Vec<Retained<AnyObject>>,
}

impl MacosBackend {
    pub(crate) fn new(_config: &PlatformConfig<'_>) -> Result<Self, Error> {
        let handler: SharedHandler = Arc::new(Mutex::new(None));
        let targets = unsafe { wire_commands(&handler) };
        Ok(Self {
            handler,
            _targets: targets,
        })
    }

    pub(crate) fn attach(&mut self, handler: EventHandler) -> Result<(), Error> {
        *self.handler.lock().unwrap() = Some(handler);
        Ok(())
    }

    pub(crate) fn set_metadata(&mut self, metadata: MediaMetadata<'_>) -> Result<(), Error> {
        unsafe {
            let center = MPNowPlayingInfoCenter::defaultCenter();
            let dict = build_now_playing_dict(&metadata);
            center.setNowPlayingInfo(Some(&dict));
        }
        Ok(())
    }

    pub(crate) fn set_playback(&mut self, playback: MediaPlayback) -> Result<(), Error> {
        unsafe {
            let center = MPNowPlayingInfoCenter::defaultCenter();
            let state = match playback {
                MediaPlayback::Stopped => MPNowPlayingPlaybackState::Stopped,
                MediaPlayback::Paused { .. } => MPNowPlayingPlaybackState::Paused,
                MediaPlayback::Playing { .. } => MPNowPlayingPlaybackState::Playing,
            };
            center.setPlaybackState(state);

            let progress = match playback {
                MediaPlayback::Stopped => None,
                MediaPlayback::Paused { progress } | MediaPlayback::Playing { progress } => {
                    progress
                }
            };
            if let Some(progress) = progress {
                update_elapsed_playback_time(progress.0);
            }
        }
        Ok(())
    }
}

/// Wire every command we care about and return the retained target handles.
unsafe fn wire_commands(handler: &SharedHandler) -> Vec<Retained<AnyObject>> {
    let mut targets: Vec<Retained<AnyObject>> = Vec::with_capacity(8);
    let center = unsafe { MPRemoteCommandCenter::sharedCommandCenter() };

    type CmdAccessor = fn(&MPRemoteCommandCenter) -> Retained<MPRemoteCommand>;
    let simple: &[(MediaControlEvent, CmdAccessor)] = &[
        (MediaControlEvent::Play, |c| unsafe { c.playCommand() }),
        (MediaControlEvent::Pause, |c| unsafe { c.pauseCommand() }),
        (MediaControlEvent::Toggle, |c| unsafe { c.togglePlayPauseCommand() }),
        (MediaControlEvent::Stop, |c| unsafe { c.stopCommand() }),
        (MediaControlEvent::Next, |c| unsafe { c.nextTrackCommand() }),
        (MediaControlEvent::Previous, |c| unsafe { c.previousTrackCommand() }),
    ];

    for (event, accessor) in simple {
        let h = handler.clone();
        let event = event.clone();
        let block = RcBlock::new(
            move |_event_ptr: std::ptr::NonNull<MPRemoteCommandEvent>| -> MPRemoteCommandHandlerStatus {
                dispatch(&h, event.clone());
                MPRemoteCommandHandlerStatus::Success
            },
        );
        let cmd = accessor(&center);
        unsafe { cmd.setEnabled(true) };
        let target = unsafe { cmd.addTargetWithHandler(&block) };
        targets.push(target);
    }

    // changePlaybackPositionCommand: payload is MPChangePlaybackPositionCommandEvent.
    {
        let h = handler.clone();
        let block = RcBlock::new(
            move |event_ptr: std::ptr::NonNull<MPRemoteCommandEvent>| -> MPRemoteCommandHandlerStatus {
                let event: &MPChangePlaybackPositionCommandEvent =
                    unsafe { event_ptr.cast().as_ref() };
                let pos_secs = unsafe { event.positionTime() }.max(0.0);
                dispatch(
                    &h,
                    MediaControlEvent::SetPosition(MediaPosition(Duration::from_secs_f64(
                        pos_secs,
                    ))),
                );
                MPRemoteCommandHandlerStatus::Success
            },
        );
        let cmd = unsafe { center.changePlaybackPositionCommand() };
        unsafe { cmd.setEnabled(true) };
        let target = unsafe { cmd.addTargetWithHandler(&block) };
        targets.push(target);
    }

    targets
}

fn dispatch(handler: &SharedHandler, event: MediaControlEvent) {
    if let Ok(mut guard) = handler.lock()
        && let Some(ref mut h) = *guard
    {
        h(event);
    }
}

unsafe fn build_now_playing_dict(metadata: &MediaMetadata<'_>) -> Retained<InfoDict> {
    let dict: Retained<InfoDict> = NSMutableDictionary::new();
    if let Some(title) = metadata.title {
        put_string(&dict, unsafe { MPMediaItemPropertyTitle }, title);
    }
    if let Some(artist) = metadata.artist {
        put_string(&dict, unsafe { MPMediaItemPropertyArtist }, artist);
    }
    if let Some(album) = metadata.album {
        put_string(&dict, unsafe { MPMediaItemPropertyAlbumTitle }, album);
    }
    if let Some(duration) = metadata.duration {
        put_number(
            &dict,
            unsafe { MPMediaItemPropertyPlaybackDuration },
            duration.as_secs_f64(),
        );
    }
    // Cover artwork is intentionally omitted: souvlaki's implementation
    // dragged in `NSImage` + `core-graphics 0.22`. Re-add via an async loader
    // once it shows up in real-world telemetry.
    let _ = metadata.cover_url;
    dict
}

fn put_string(dict: &InfoDict, key: &NSString, value: &str) {
    let ns_value = NSString::from_str(value);
    // SAFETY: `&NSString` upcasts losslessly to `&AnyObject` via deref;
    // `insert` takes &ObjectType (= AnyObject) and copies the key.
    let any: &AnyObject = ns_value.as_ref();
    dict.insert(key, any);
}

fn put_number(dict: &InfoDict, key: &NSString, value: f64) {
    let ns_value = NSNumber::numberWithDouble(value);
    let any: &AnyObject = ns_value.as_ref();
    dict.insert(key, any);
}

/// Update only the elapsed-playback-time entry without rebuilding metadata.
unsafe fn update_elapsed_playback_time(progress: Duration) {
    unsafe {
        let center = MPNowPlayingInfoCenter::defaultCenter();
        let dict: Retained<InfoDict> = NSMutableDictionary::new();
        if let Some(current) = center.nowPlayingInfo() {
            dict.addEntriesFromDictionary(&current);
        }
        put_number(
            &dict,
            MPNowPlayingInfoPropertyElapsedPlaybackTime,
            progress.as_secs_f64(),
        );
        center.setNowPlayingInfo(Some(&dict));
    }
}
