//! Linux / FreeBSD backend driving an MPRIS player over D-Bus.
//!
//! `mpris-server` is async-first. We hide that behind a sync API by spinning
//! up a tokio current-thread runtime in a dedicated background thread,
//! marshalling commands in via an `mpsc` channel and events out via the
//! caller-supplied `EventHandler`.

use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use mpris_server::{
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus, Player, Property, Time, TrackId, Volume,
};
use tokio::sync::mpsc;

use crate::{
    Error, MediaControlEvent, MediaMetadata, MediaPlayback, MediaPosition, SeekDirection,
    backend::EventHandler, types::PlatformConfig,
};

type SharedHandler = Arc<Mutex<Option<EventHandler>>>;

/// Owned snapshot of metadata so we can send it across threads.
#[derive(Default, Debug, Clone)]
struct OwnedMetadata {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<Duration>,
    cover_url: Option<String>,
}

impl OwnedMetadata {
    fn from_borrowed(m: &MediaMetadata<'_>) -> Self {
        Self {
            title: m.title.map(str::to_owned),
            artist: m.artist.map(str::to_owned),
            album: m.album.map(str::to_owned),
            duration: m.duration,
            cover_url: m.cover_url.map(str::to_owned),
        }
    }
}

/// Commands sent from the public API into the runtime thread.
enum Cmd {
    SetMetadata(OwnedMetadata),
    SetPlayback(MediaPlayback),
    Shutdown,
}

pub(crate) struct MprisBackend {
    handler: SharedHandler,
    cmd_tx: mpsc::UnboundedSender<Cmd>,
    runtime_thread: Option<JoinHandle<()>>,
}

impl MprisBackend {
    pub(crate) fn new(config: &PlatformConfig<'_>) -> Result<Self, Error> {
        let handler: SharedHandler = Arc::new(Mutex::new(None));
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Cmd>();
        // Hand-off channel so `new()` can surface init errors synchronously.
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);

        let dbus_name = format!("org.mpris.MediaPlayer2.{}", config.dbus_name);
        let identity = config.display_name.to_owned();
        let handler_for_thread = handler.clone();

        let runtime_thread = std::thread::Builder::new()
            .name("sotf-mpris".to_string())
            .spawn(move || {
                run_mpris_thread(dbus_name, identity, handler_for_thread, cmd_rx, init_tx);
            })
            .map_err(|e| Error::Init(format!("spawn mpris thread: {e}")))?;

        match init_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                handler,
                cmd_tx,
                runtime_thread: Some(runtime_thread),
            }),
            Ok(Err(msg)) => Err(Error::Init(msg)),
            Err(e) => Err(Error::Init(format!("mpris init channel: {e}"))),
        }
    }

    pub(crate) fn attach(&mut self, handler: EventHandler) -> Result<(), Error> {
        *self.handler.lock().unwrap() = Some(handler);
        Ok(())
    }

    pub(crate) fn set_metadata(&mut self, metadata: MediaMetadata<'_>) -> Result<(), Error> {
        self.cmd_tx
            .send(Cmd::SetMetadata(OwnedMetadata::from_borrowed(&metadata)))
            .map_err(|e| Error::Update(format!("mpris cmd send: {e}")))
    }

    pub(crate) fn set_playback(&mut self, playback: MediaPlayback) -> Result<(), Error> {
        self.cmd_tx
            .send(Cmd::SetPlayback(playback))
            .map_err(|e| Error::Update(format!("mpris cmd send: {e}")))
    }
}

impl Drop for MprisBackend {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(Cmd::Shutdown);
        if let Some(handle) = self.runtime_thread.take() {
            let _ = handle.join();
        }
    }
}

fn run_mpris_thread(
    dbus_name: String,
    identity: String,
    handler: SharedHandler,
    mut cmd_rx: mpsc::UnboundedReceiver<Cmd>,
    init_tx: std::sync::mpsc::SyncSender<Result<(), String>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            let _ = init_tx.send(Err(format!("tokio runtime: {e}")));
            return;
        }
    };

    // `Player::run()` yields a `!Send` future driven on this thread, and we use
    // `spawn_local` to run it concurrently with the command loop — both require a
    // `LocalSet` rather than the bare current-thread runtime.
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async move {
        let player = match build_player(&dbus_name, &identity, handler).await {
            Ok(p) => p,
            Err(e) => {
                let _ = init_tx.send(Err(format!("build player: {e}")));
                return;
            }
        };

        let player = Arc::new(player);
        // Player::run returns a future that drives the D-Bus server. Spawn it
        // so command processing can happen in parallel.
        let server_task = {
            let player = player.clone();
            tokio::task::spawn_local(async move { player.run().await })
        };

        let _ = init_tx.send(Ok(()));

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                Cmd::SetMetadata(m) => apply_metadata(&player, m).await,
                Cmd::SetPlayback(pb) => apply_playback(&player, pb).await,
                Cmd::Shutdown => break,
            }
        }

        server_task.abort();
    });
}

async fn build_player(
    dbus_name: &str,
    identity: &str,
    handler: SharedHandler,
) -> Result<Player, Box<dyn std::error::Error + Send + Sync>> {
    let player = Player::builder(dbus_name)
        .identity(identity.to_string())
        .can_play(true)
        .can_pause(true)
        .can_go_next(true)
        .can_go_previous(true)
        .can_seek(true)
        .can_control(true)
        .build()
        .await?;

    // Invoke the user closure with the mutex released so callers can
    // safely re-enter `MediaControls::set_metadata` / `set_playback` from
    // inside the handler without self-deadlock. The `take`/restore dance
    // temporarily removes ownership of the boxed FnMut while it runs;
    // concurrent dispatchers during that window drop their events, which
    // matches the "best-effort, low-rate" contract.
    let dispatch_ev = move |handler: &SharedHandler, ev: MediaControlEvent| {
        let taken = match handler.lock() {
            Ok(mut guard) => guard.take(),
            Err(_) => return,
        };
        let Some(mut h) = taken else { return };
        h(ev);
        if let Ok(mut guard) = handler.lock() {
            // If an `attach` slid in while the user closure ran, keep the
            // newer one — otherwise restore the boxed closure.
            if guard.is_none() {
                *guard = Some(h);
            }
        }
    };

    {
        let h = handler.clone();
        player.connect_play(move |_| dispatch_ev(&h, MediaControlEvent::Play));
    }
    {
        let h = handler.clone();
        player.connect_pause(move |_| dispatch_ev(&h, MediaControlEvent::Pause));
    }
    {
        let h = handler.clone();
        player.connect_play_pause(move |_| dispatch_ev(&h, MediaControlEvent::Toggle));
    }
    {
        let h = handler.clone();
        player.connect_stop(move |_| dispatch_ev(&h, MediaControlEvent::Stop));
    }
    {
        let h = handler.clone();
        player.connect_next(move |_| dispatch_ev(&h, MediaControlEvent::Next));
    }
    {
        let h = handler.clone();
        player.connect_previous(move |_| dispatch_ev(&h, MediaControlEvent::Previous));
    }
    {
        let h = handler.clone();
        player.connect_seek(move |_, time: Time| {
            // MPRIS Seek offset is signed micros.
            let micros = time.as_micros();
            let abs = Duration::from_micros(micros.unsigned_abs());
            let dir = if micros >= 0 {
                SeekDirection::Forward
            } else {
                SeekDirection::Backward
            };
            dispatch_ev(&h, MediaControlEvent::SeekBy(dir, abs));
        });
    }
    {
        let h = handler.clone();
        player.connect_set_position(move |_, _track: &TrackId, time: Time| {
            let micros = nonnegative_time_micros(time);
            dispatch_ev(
                &h,
                MediaControlEvent::SetPosition(MediaPosition(Duration::from_micros(micros))),
            );
        });
    }
    {
        let h = handler.clone();
        player.connect_set_volume(move |_, vol: Volume| {
            dispatch_ev(&h, MediaControlEvent::SetVolume(vol.clamp(0.0, 1.0)));
        });
    }
    {
        let h = handler.clone();
        player.connect_raise(move |_| dispatch_ev(&h, MediaControlEvent::Raise));
    }
    {
        let h = handler.clone();
        player.connect_quit(move |_| dispatch_ev(&h, MediaControlEvent::Quit));
    }
    {
        let h = handler.clone();
        player.connect_open_uri(move |_, uri: &str| {
            dispatch_ev(&h, MediaControlEvent::OpenUri(uri.to_owned()));
        });
    }

    // Defaults required by MPRIS even though we don't expose loop/shuffle.
    let _ = LoopStatus::None;
    let _ = PlaybackRate::default();
    let _ = Property::PlaybackStatus(PlaybackStatus::Stopped);

    Ok(player)
}

async fn apply_metadata(player: &Player, m: OwnedMetadata) {
    let mut md = Metadata::new();
    if let Some(t) = m.title {
        md.set_title(Some(t));
    }
    if let Some(a) = m.artist {
        md.set_artist(Some([a]));
    }
    if let Some(al) = m.album {
        md.set_album(Some(al));
    }
    if let Some(dur) = m.duration {
        md.set_length(Some(duration_to_time(dur)));
    }
    if let Some(url) = m.cover_url {
        md.set_art_url(Some(url));
    }
    if let Err(e) = player.set_metadata(md).await {
        log::warn!("mpris set_metadata: {e}");
    }
}

async fn apply_playback(player: &Player, pb: MediaPlayback) {
    let (status, progress) = match pb {
        MediaPlayback::Stopped => (PlaybackStatus::Stopped, None),
        MediaPlayback::Paused { progress } => (PlaybackStatus::Paused, progress),
        MediaPlayback::Playing { progress } => (PlaybackStatus::Playing, progress),
    };
    if let Err(e) = player.set_playback_status(status).await {
        log::warn!("mpris set_playback_status: {e}");
    }
    if let Some(MediaPosition(d)) = progress {
        player.set_position(duration_to_time(d));
    }
}

fn duration_to_time(duration: Duration) -> Time {
    // Guard against overflow: `Duration::as_micros` returns u128. Clamp
    // to `i64::MAX` (≈292 000 years) so we never wrap into a negative
    // MPRIS `Time`.
    let micros = duration.as_micros().min(i64::MAX as u128) as i64;
    Time::from_micros(micros)
}

fn nonnegative_time_micros(time: Time) -> u64 {
    u64::try_from(time.as_micros().max(0)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_to_time_clamps_overflow() {
        let time = duration_to_time(Duration::from_secs(u64::MAX));

        assert_eq!(time.as_micros(), i64::MAX);
    }

    #[test]
    fn nonnegative_time_micros_clamps_negative_offsets() {
        assert_eq!(nonnegative_time_micros(Time::from_micros(-42)), 0);
    }

    #[test]
    fn nonnegative_time_micros_preserves_positive_offsets() {
        assert_eq!(nonnegative_time_micros(Time::from_micros(42)), 42);
    }
}
