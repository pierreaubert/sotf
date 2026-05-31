//! macOS backend.
//!
//! Drives `MPRemoteCommandCenter` (incoming media-key events) and
//! `MPNowPlayingInfoCenter` (now-playing metadata + playback state) through
//! the modern `objc2-media-player` bindings — no `cocoa-rs` dependency.
//!
//! Thread affinity. `MPNowPlayingInfoCenter` and `MPRemoteCommandCenter`
//! mutating APIs require execution on the main thread (or at least a thread
//! with a running `CFRunLoop`). Command wiring in `MediaControls::new` is
//! therefore rejected off-main. Later metadata/playback writes are marshalled
//! onto `dispatch_get_main_queue` via `dispatch2::DispatchQueue::main().exec_async`.

#![allow(
    unsafe_code,
    reason = "objc2 framework calls are inherently unsafe FFI"
)]

use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use block2::RcBlock;
use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, Message};
use objc2_foundation::{NSMutableDictionary, NSNumber, NSString};
use objc2_media_player::{
    MPChangePlaybackPositionCommandEvent, MPMediaItemPropertyAlbumTitle, MPMediaItemPropertyArtist,
    MPMediaItemPropertyPlaybackDuration, MPMediaItemPropertyTitle, MPNowPlayingInfoCenter,
    MPNowPlayingInfoPropertyElapsedPlaybackTime, MPNowPlayingPlaybackState, MPRemoteCommand,
    MPRemoteCommandCenter, MPRemoteCommandEvent, MPRemoteCommandHandlerStatus,
};

use crate::{
    Error, MediaControlEvent, MediaMetadata, MediaPlayback, MediaPosition, backend::EventHandler,
    types::PlatformConfig,
};

/// Shared event-dispatch handle. Cloned into every Objective-C block so the
/// blocks (which the OS holds for the lifetime of the process) can call back
/// into the user closure even after `attach()` swaps it.
type SharedHandler = Arc<Mutex<Option<EventHandler>>>;

/// Concrete metadata-dictionary type expected by `MPNowPlayingInfoCenter`.
type InfoDict = NSMutableDictionary<NSString, AnyObject>;

/// Owned metadata snapshot (plain Rust types so it crosses threads cheaply
/// — `Retained<NSString>` is not `Send`, but `String` is).
#[derive(Default, Debug, Clone)]
struct OwnedMetadata {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<Duration>,
}

impl OwnedMetadata {
    fn from_borrowed(m: &MediaMetadata<'_>) -> Self {
        Self {
            title: m.title.map(str::to_owned),
            artist: m.artist.map(str::to_owned),
            album: m.album.map(str::to_owned),
            duration: m.duration,
        }
    }
}

/// `Retained<T>` wrapper with an `unsafe` `Send` impl.
///
/// SAFETY: We only wrap pointers that refer to thread-safe Objective-C
/// objects: `MPRemoteCommand` (process-wide singleton dispensed by
/// `MPRemoteCommandCenter`) and the opaque `id` token returned by
/// `addTargetWithHandler:`. Both have thread-safe `retain` / `release`
/// implementations in `MediaPlayer.framework`. The wrapper is used to
/// shuttle the retained handles into a main-queue `exec_async` block from
/// the `Drop` impl — sound because the wrapper never escapes that block.
struct SendRetained<T: Message>(Retained<T>);

// SAFETY: see the doc comment on `SendRetained`.
unsafe impl<T: Message> Send for SendRetained<T> {}

/// Wired command/target pair retained for the lifetime of the backend.
struct WiredCommand {
    cmd: SendRetained<MPRemoteCommand>,
    target: SendRetained<AnyObject>,
}

thread_local! {
    /// Cached `Retained<InfoDict>` *owned by the main thread*. We never
    /// read or mutate this outside `DispatchQueue::main().exec_async`, so
    /// a thread-local is the cheapest safe storage — no `Send` gymnastics,
    /// no `MainThreadBound` ceremony, no cross-thread `Retained` traffic.
    /// Successive `set_playback` ticks mutate this same dict in place,
    /// avoiding the per-tick round trip through
    /// `MPNowPlayingInfoCenter::nowPlayingInfo()` /
    /// `addEntriesFromDictionary:` that the original implementation paid.
    static CACHED_DICT: RefCell<Option<Retained<InfoDict>>> = const { RefCell::new(None) };
}

pub(crate) struct MacosBackend {
    handler: SharedHandler,
    /// Commands + target tokens for the wired `MPRemoteCommandCenter`
    /// selectors. Held alive for the lifetime of the backend so the OS
    /// can keep dispatching events; detached in `Drop` via `removeTarget:`.
    wired: Vec<WiredCommand>,
}

impl MacosBackend {
    pub(crate) fn new(_config: &PlatformConfig<'_>) -> Result<Self, Error> {
        let Some(_mtm) = MainThreadMarker::new() else {
            return Err(Error::Init(
                "macOS media controls must be constructed on the main thread".to_string(),
            ));
        };
        let handler: SharedHandler = Arc::new(Mutex::new(None));
        // SAFETY: we just proved we are on the main thread, and `handler`
        // is owned by the returned backend until `Drop` removes the targets.
        let wired = unsafe { wire_commands(&handler) };
        Ok(Self { handler, wired })
    }

    pub(crate) fn attach(&mut self, handler: EventHandler) -> Result<(), Error> {
        *self.handler.lock().unwrap() = Some(handler);
        Ok(())
    }

    pub(crate) fn set_metadata(&mut self, metadata: MediaMetadata<'_>) -> Result<(), Error> {
        // Cover artwork is intentionally omitted: souvlaki's implementation
        // dragged in `NSImage` + `core-graphics`. Re-add via an async
        // loader once it shows up in real-world telemetry.
        let _ = metadata.cover_url;
        let owned = OwnedMetadata::from_borrowed(&metadata);
        // Marshal onto the main thread — `MPNowPlayingInfoCenter`
        // mutators are main-thread-affine. `exec_async` is fire-and-forget;
        // if the caller already *is* on the main thread, the block is
        // enqueued rather than recursively invoked (no surprise
        // re-entrancy).
        DispatchQueue::main().exec_async(move || {
            // SAFETY: closure runs on the main queue.
            unsafe {
                let dict = build_now_playing_dict(&owned);
                CACHED_DICT.with(|slot| *slot.borrow_mut() = Some(dict.clone()));
                MPNowPlayingInfoCenter::defaultCenter().setNowPlayingInfo(Some(&dict));
            }
        });
        Ok(())
    }

    pub(crate) fn set_playback(&mut self, playback: MediaPlayback) -> Result<(), Error> {
        let state = match playback {
            MediaPlayback::Stopped => MPNowPlayingPlaybackState::Stopped,
            MediaPlayback::Paused { .. } => MPNowPlayingPlaybackState::Paused,
            MediaPlayback::Playing { .. } => MPNowPlayingPlaybackState::Playing,
        };
        let progress = match playback {
            MediaPlayback::Stopped => None,
            MediaPlayback::Paused { progress } | MediaPlayback::Playing { progress } => progress,
        };
        DispatchQueue::main().exec_async(move || {
            // SAFETY: closure runs on the main queue, which is where
            // `MPNowPlayingInfoCenter` mutators are required to run.
            unsafe {
                MPNowPlayingInfoCenter::defaultCenter().setPlaybackState(state);
                if let Some(MediaPosition(d)) = progress {
                    update_elapsed_playback_time(d);
                }
            }
        });
        Ok(())
    }
}

impl Drop for MacosBackend {
    fn drop(&mut self) {
        // Detach every registered block from the shared command center so
        // future `MediaControls` instances don't double-dispatch through
        // ghost blocks. `removeTarget:` must run on the main thread; we
        // go through `exec_async` regardless of the dropping thread to
        // keep the threading contract uniform. The wired handles move
        // into the closure and drop there.
        let wired = std::mem::take(&mut self.wired);
        if wired.is_empty() {
            return;
        }
        DispatchQueue::main().exec_async(move || {
            for w in &wired {
                // SAFETY: main queue + the `MPRemoteCommand` handle is
                // held alive by `wired`; `removeTarget:` accepts the same
                // target returned by `addTargetWithHandler:` (per Apple
                // docs).
                unsafe {
                    w.cmd.0.removeTarget(Some(&w.target.0));
                    w.cmd.0.setEnabled(false);
                }
            }
            drop(wired);
        });
    }
}

/// Wire every command we care about and return the retained command/target
/// pairs.
///
/// # Safety
///
/// Caller must run this on the main thread and guarantee that the
/// `SharedHandler` outlives the registered blocks. The current design ties
/// that lifetime to the `MacosBackend`, whose `Drop` impl detaches the
/// blocks before the `Arc` ref-count can drop to zero on the data the blocks
/// captured.
unsafe fn wire_commands(handler: &SharedHandler) -> Vec<WiredCommand> {
    let mut wired: Vec<WiredCommand> = Vec::with_capacity(8);
    // SAFETY: this function is only called on the main thread.
    let center = unsafe { MPRemoteCommandCenter::sharedCommandCenter() };

    type CmdAccessor = fn(&MPRemoteCommandCenter) -> Retained<MPRemoteCommand>;
    let simple: &[(MediaControlEvent, CmdAccessor)] = &[
        (MediaControlEvent::Play, |c| unsafe { c.playCommand() }),
        (MediaControlEvent::Pause, |c| unsafe { c.pauseCommand() }),
        (MediaControlEvent::Toggle, |c| unsafe {
            c.togglePlayPauseCommand()
        }),
        (MediaControlEvent::Stop, |c| unsafe { c.stopCommand() }),
        (MediaControlEvent::Next, |c| unsafe { c.nextTrackCommand() }),
        (MediaControlEvent::Previous, |c| unsafe {
            c.previousTrackCommand()
        }),
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
        // SAFETY: command object lives on the shared command-center singleton.
        unsafe { cmd.setEnabled(true) };
        let target = unsafe { cmd.addTargetWithHandler(&block) };
        wired.push(WiredCommand {
            cmd: SendRetained(cmd),
            target: SendRetained(target),
        });
    }

    // `changePlaybackPositionCommand` returns an
    // `MPChangePlaybackPositionCommand` — a documented subclass of
    // `MPRemoteCommand`. Up-cast via `Retained::into_super` so the
    // wired-command store can be uniformly typed.
    {
        let h = handler.clone();
        let block = RcBlock::new(
            move |event_ptr: std::ptr::NonNull<MPRemoteCommandEvent>| -> MPRemoteCommandHandlerStatus {
                // SAFETY: the OS only invokes this block for
                // `changePlaybackPositionCommand`, whose event payload is
                // statically known to be
                // `MPChangePlaybackPositionCommandEvent` (Apple docs:
                // MPRemoteCommandCenter.changePlaybackPositionCommand).
                let event: &MPChangePlaybackPositionCommandEvent =
                    unsafe { event_ptr.cast().as_ref() };
                // SAFETY: `positionTime` is a `@property NSTimeInterval`
                // on the event and safe to read from any thread.
                let pos_secs = unsafe { event.positionTime() };
                // Sanitise NaN / infinity here — the OS occasionally
                // forwards bogus drag-end events, and
                // `Duration::from_secs_f64(NaN)` panics.
                let position = MediaPosition::from_secs_f64(pos_secs);
                dispatch(&h, MediaControlEvent::SetPosition(position));
                MPRemoteCommandHandlerStatus::Success
            },
        );
        let sub_cmd = unsafe { center.changePlaybackPositionCommand() };
        unsafe { sub_cmd.setEnabled(true) };
        let target = unsafe { sub_cmd.addTargetWithHandler(&block) };
        let cmd: Retained<MPRemoteCommand> = sub_cmd.into_super();
        wired.push(WiredCommand {
            cmd: SendRetained(cmd),
            target: SendRetained(target),
        });
    }

    wired
}

/// Invoke the user closure with the handler mutex released.
///
/// `FnMut` requires `&mut`, so we `take()` the boxed closure out of the
/// shared slot, run it without holding the lock, then put it back. This
/// lets the user closure safely re-enter `MediaControls::set_metadata` /
/// `set_playback` (which queue work via the OS callback path) without
/// self-deadlock.
fn dispatch(handler: &SharedHandler, event: MediaControlEvent) {
    let taken = match handler.lock() {
        Ok(mut guard) => guard.take(),
        Err(_) => return,
    };
    let Some(mut h) = taken else { return };
    h(event);
    if let Ok(mut guard) = handler.lock() {
        // If an `attach` slid in while the user closure ran, keep the
        // newer one — otherwise restore the boxed closure.
        if guard.is_none() {
            *guard = Some(h);
        }
    }
}

/// Build a full now-playing dictionary from owned metadata.
///
/// # Safety
///
/// Must be called on the main thread.
unsafe fn build_now_playing_dict(metadata: &OwnedMetadata) -> Retained<InfoDict> {
    let dict: Retained<InfoDict> = NSMutableDictionary::new();
    if let Some(title) = metadata.title.as_deref() {
        // SAFETY: `MPMediaItemPropertyTitle` is a static `NSString*`
        // symbol.
        put_string(&dict, unsafe { MPMediaItemPropertyTitle }, title);
    }
    if let Some(artist) = metadata.artist.as_deref() {
        put_string(&dict, unsafe { MPMediaItemPropertyArtist }, artist);
    }
    if let Some(album) = metadata.album.as_deref() {
        put_string(&dict, unsafe { MPMediaItemPropertyAlbumTitle }, album);
    }
    if let Some(duration) = metadata.duration {
        put_number(
            &dict,
            unsafe { MPMediaItemPropertyPlaybackDuration },
            duration.as_secs_f64(),
        );
    }
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
///
/// Uses the thread-local main-queue cache so we mutate the **same** dict
/// across successive ticks instead of round-tripping through
/// `MPNowPlayingInfoCenter::nowPlayingInfo()` (which copies all entries
/// out of Apple's internal cache).
///
/// # Safety
///
/// Must be called on the main thread.
unsafe fn update_elapsed_playback_time(progress: Duration) {
    CACHED_DICT.with(|slot| {
        let mut borrow = slot.borrow_mut();
        if borrow.is_none() {
            // First playback update before metadata: lazy-init an empty
            // dict so the elapsed time still shows on the lock screen.
            *borrow = Some(NSMutableDictionary::new());
        }
        let dict = borrow.as_ref().expect("CACHED_DICT just initialised");
        // SAFETY: `MPNowPlayingInfoPropertyElapsedPlaybackTime` is a
        // static `NSString*` constant.
        put_number(
            dict,
            unsafe { MPNowPlayingInfoPropertyElapsedPlaybackTime },
            progress.as_secs_f64(),
        );
        // SAFETY: main thread.
        unsafe {
            MPNowPlayingInfoCenter::defaultCenter().setNowPlayingInfo(Some(dict));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_refuses_off_main_thread() {
        let handle = std::thread::spawn(|| {
            let cfg = PlatformConfig {
                dbus_name: "test",
                display_name: "Test",
                hwnd: None,
            };
            MacosBackend::new(&cfg)
        });
        let result = handle.join().expect("spawned thread panicked");
        match result {
            Err(Error::Init(msg)) => assert!(msg.contains("main thread")),
            Ok(_) => panic!("MacosBackend::new unexpectedly succeeded off-main"),
            Err(err) => panic!("unexpected error: {err:?}"),
        }
    }

    /// Regression guard: the `MPChangePlaybackPosition` block routes the
    /// position value through `MediaPosition::from_secs_f64`, which must
    /// not panic on NaN (the OS occasionally forwards bogus drag-end
    /// events).
    #[test]
    fn macos_position_from_nan_does_not_panic() {
        let p = MediaPosition::from_secs_f64(f64::NAN);
        assert_eq!(p.0, Duration::ZERO);
    }
}
