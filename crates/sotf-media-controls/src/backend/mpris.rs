//! Linux / FreeBSD backend driving an MPRIS player over D-Bus.
//!
//! `mpris-server` is async-first. We hide that behind a sync API by spinning
//! up a tokio current-thread runtime in a dedicated background thread,
//! marshalling commands in via an `mpsc` channel and events out via the
//! caller-supplied `EventHandler`.

use std::sync::Arc;
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

/// Commands sent from the public API into the runtime thread.
enum Cmd {
    SetMetadata(OwnedMetadata),
    SetPlayback(MediaPlayback),
    AttachHandler(EventHandler),
    DispatchEvent(MediaControlEvent),
    Shutdown,
}

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

pub(crate) struct MprisBackend {
    cmd_tx: mpsc::UnboundedSender<Cmd>,
    runtime_thread: Option<JoinHandle<()>>,
}

impl MprisBackend {
    pub(crate) fn new(config: &PlatformConfig<'_>) -> Result<Self, Error> {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Cmd>();
        // Hand-off channel so `new()` can surface init errors synchronously.
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);

        let dbus_name = format!("org.mpris.MediaPlayer2.{}", config.dbus_name);
        let identity = config.display_name.to_owned();
        let cmd_tx_for_thread = cmd_tx.clone();

        let runtime_thread = std::thread::Builder::new()
            .name("sotf-mpris".to_string())
            .spawn(move || {
                run_mpris_thread(dbus_name, identity, cmd_tx_for_thread, cmd_rx, init_tx);
            })
            .map_err(|e| Error::Init(format!("spawn mpris thread: {e}")))?;

        match init_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                cmd_tx,
                runtime_thread: Some(runtime_thread),
            }),
            Ok(Err(msg)) => Err(Error::Init(msg)),
            Err(e) => Err(Error::Init(format!("mpris init channel: {e}"))),
        }
    }

    pub(crate) fn attach(&mut self, handler: EventHandler) -> Result<(), Error> {
        self.cmd_tx
            .send(Cmd::AttachHandler(handler))
            .map_err(|e| Error::Attach(format!("mpris attach: {e}")))
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
        // 1. Signal the runtime thread to exit and wait for it. This drops
        //    the user-supplied `EventHandler` (and any captured state) before
        //    the backend is destroyed.
        // 2. The tokio runtime is driven inside the joined thread, so all
        //    D-Bus callbacks stop before this function returns.
        let _ = self.cmd_tx.send(Cmd::Shutdown);
        if let Some(handle) = self.runtime_thread.take() {
            let _ = handle.join();
        }
    }
}

fn run_mpris_thread(
    dbus_name: String,
    identity: String,
    cmd_tx: mpsc::UnboundedSender<Cmd>,
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
        let player = match build_player(&dbus_name, &identity, cmd_tx).await {
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

        let mut handler: Option<EventHandler> = None;
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                Cmd::SetMetadata(m) => apply_metadata(&player, m).await,
                Cmd::SetPlayback(pb) => apply_playback(&player, pb).await,
                Cmd::AttachHandler(h) => handler = Some(h),
                Cmd::DispatchEvent(ev) => {
                    if let Some(ref mut h) = handler {
                        h(ev);
                    }
                }
                Cmd::Shutdown => break,
            }
        }

        server_task.abort();
    });
}

async fn build_player(
    dbus_name: &str,
    identity: &str,
    cmd_tx: mpsc::UnboundedSender<Cmd>,
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

    // Send control events back through the same command channel. This avoids
    // the previous `Arc<Mutex<Option<EventHandler>>>` pattern that serialized
    // every OS callback through a single mutex.
    let dispatch = move |ev: MediaControlEvent| {
        let _ = cmd_tx.send(Cmd::DispatchEvent(ev));
    };

    {
        let d = dispatch.clone();
        player.connect_play(move |_| d(MediaControlEvent::Play));
    }
    {
        let d = dispatch.clone();
        player.connect_pause(move |_| d(MediaControlEvent::Pause));
    }
    {
        let d = dispatch.clone();
        player.connect_play_pause(move |_| d(MediaControlEvent::Toggle));
    }
    {
        let d = dispatch.clone();
        player.connect_stop(move |_| d(MediaControlEvent::Stop));
    }
    {
        let d = dispatch.clone();
        player.connect_next(move |_| d(MediaControlEvent::Next));
    }
    {
        let d = dispatch.clone();
        player.connect_previous(move |_| d(MediaControlEvent::Previous));
    }
    {
        let d = dispatch.clone();
        player.connect_seek(move |_, time: Time| {
            // MPRIS Seek offset is signed micros.
            let micros = time.as_micros();
            let abs = Duration::from_micros(micros.unsigned_abs());
            let dir = if micros >= 0 {
                SeekDirection::Forward
            } else {
                SeekDirection::Backward
            };
            d(MediaControlEvent::SeekBy(dir, abs));
        });
    }
    {
        let d = dispatch.clone();
        player.connect_set_position(move |_, _track: &TrackId, time: Time| {
            let micros = nonnegative_time_micros(time);
            d(MediaControlEvent::SetPosition(MediaPosition(
                Duration::from_micros(micros),
            )));
        });
    }
    {
        let d = dispatch.clone();
        player.connect_set_volume(move |_, vol: Volume| {
            d(MediaControlEvent::SetVolume(vol.clamp(0.0, 1.0)));
        });
    }
    {
        let d = dispatch.clone();
        player.connect_raise(move |_| d(MediaControlEvent::Raise));
    }
    {
        let d = dispatch.clone();
        player.connect_quit(move |_| d(MediaControlEvent::Quit));
    }
    {
        let d = dispatch.clone();
        player.connect_open_uri(move |_, uri: &str| {
            d(MediaControlEvent::OpenUri(uri.to_owned()));
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

    #[test]
    fn duration_to_time_preserves_normal_values() {
        let time = duration_to_time(Duration::from_millis(1_500));
        assert_eq!(time.as_micros(), 1_500_000);
    }

    #[test]
    fn owned_metadata_from_borrowed_copies_strings() {
        let borrowed = MediaMetadata {
            title: Some("Title"),
            artist: Some("Artist"),
            album: Some("Album"),
            duration: Some(Duration::from_secs(180)),
            cover_url: Some("file:///cover.jpg"),
        };
        let owned = OwnedMetadata::from_borrowed(&borrowed);
        assert_eq!(owned.title, Some("Title".to_string()));
        assert_eq!(owned.artist, Some("Artist".to_string()));
        assert_eq!(owned.album, Some("Album".to_string()));
        assert_eq!(owned.duration, Some(Duration::from_secs(180)));
        assert_eq!(owned.cover_url, Some("file:///cover.jpg".to_string()));
    }
}
