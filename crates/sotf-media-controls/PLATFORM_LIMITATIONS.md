# Platform Limitations — `sotf-media-controls`

This crate provides OS media-control integration (Now Playing / MPRIS / SMTC)
for SOTF apps. Platform support is intentionally narrow and gated by OS
capabilities.

## Supported platforms

| Platform | Backend | Status |
|----------|---------|--------|
| macOS    | `MPRemoteCommandCenter` + `MPNowPlayingInfoCenter` via `objc2-media-player` | Fully supported |
| Linux    | MPRIS via `mpris-server` (zbus / D-Bus) | Supported |
| FreeBSD  | MPRIS via `mpris-server` (zbus / D-Bus) | Supported |
| Windows  | System Media Transport Controls (SMTC) | **Not implemented** — `MediaControls::new` returns `Error::Unsupported` |
| iOS      | — | **Unsupported** — `MediaControls::new` returns `Error::Unsupported` |
| tvOS     | — | **Unsupported** — `MediaControls::new` returns `Error::Unsupported` |
| Other    | — | **Unsupported** — `MediaControls::new` returns `Error::Unsupported` |

## macOS-specific assumptions and limitations

- **Main-thread construction is required.** `MediaControls::new` must be called
  from the main thread. Calling it from a background thread returns
  `Error::Init("...must be constructed on the main thread")`.
- **Main-thread mutation.** `set_metadata` and `set_playback` are marshalled
  onto `dispatch_get_main_queue` via `dispatch2`. The public methods may be
  called from any thread.
- **No cover artwork.** Lock-screen / Control Center artwork is intentionally
  omitted to avoid a `NSImage` / `core-graphics` dependency. Re-adding it is a
  tracked future improvement.
- **Command-center targets are process-global.** `MPRemoteCommandCenter` is a
  process singleton. Constructing multiple `MediaControls` instances in the same
  process will overwrite the previous targets; only the most recently attached
  handler receives events.
- **Lifetime contract.** The user handler is `'static` and owned by the
  `MacosBackend`. Dropping the backend joins the handler thread and removes
  command-center targets. If a macOS block is in flight when the backend is
  dropped, its event send is silently dropped because the handler thread is
  already gone.

## Linux / FreeBSD-specific assumptions and limitations

- **D-Bus session bus required.** `MprisBackend::new` starts an MPRIS player on
  the session bus. If D-Bus is unavailable, initialization fails with
  `Error::Init`.
- **Dedicated tokio current-thread runtime.** The backend spawns a background
  thread running a tokio `current_thread` runtime inside a `LocalSet`.
- **Lifetime contract.** The user handler is `'static` and owned by the MPRIS
  runtime thread. Dropping the backend sends `Cmd::Shutdown` and joins the
  runtime thread, so the handler is dropped before app/player state can be
  destroyed.

## Delivery semantics

- **Fire-and-forget updates.** `set_metadata` / `set_playback` queue the
  update on the platform backend and return `Ok(())` once queued. On macOS
  the update is applied asynchronously on the main queue and may be dropped
  while the process is tearing down; callers cannot distinguish an applied
  update from a dropped one. An `Err` is returned only when the backend
  itself is already gone.
- **Best-effort macOS detach.** `Drop` removes command-center targets via an
  async main-queue block. If the process exits before the queue drains,
  `removeTarget:` never runs and stale targets may double-dispatch into the
  next `MediaControls` in the same process. Restart the process to recover.
- **`Drop` may block.** Both backends join their callback/runtime thread on
  drop (macOS handler thread, MPRIS tokio thread), so dropping the handle
  can block briefly, e.g. while a D-Bus call drains.

## Incoming event coverage

Which `MediaControlEvent`s each backend can produce:

| Event | macOS | MPRIS (Linux / FreeBSD) |
|-------|-------|-------------------------|
| Play / Pause / Toggle / Stop / Next / Previous | wired | wired |
| SetPosition | wired (`changePlaybackPositionCommand`) | wired; clients offer it only when `MprisCapabilities::seek` is advertised |
| SetVolume | not forwarded by the OS | wired |
| SeekBy | no skip-interval command wired | wired |
| Seek | produced by neither backend (MPRIS skips arrive as `SeekBy`); API completeness only | same |
| Raise / Quit / OpenUri | n/a | wired; the app must handle them (see `types.rs`) |

## MPRIS capability advertisement

`PlatformConfig::mpris_capabilities` gates the `can_play` / `can_pause` /
`can_go_next` / `can_go_previous` / `can_seek` / `can_control` flags
advertised over D-Bus. The default (`MprisCapabilities::all`) preserves the
previous always-on behavior. macOS ignores this field and always wires its
fixed command set. Cover art (`cover_url`) is honored on MPRIS but
intentionally omitted on macOS (see below).

## General lifetime requirements

- The closure passed to `MediaControls::attach` must be `Send + 'static`.
  Borrowing app or player state directly will not compile; capture an
  `Arc<...>` or a channel sender instead.
- `MediaControls` must outlive any OS callbacks. Dropping the handle before the
  app exits is safe because the backend joins/stops its callback machinery in
  `Drop`.

## Testing gaps

- Real macOS media-key hardware is **not** required for the unit-test suite.
- macOS backend tests exercise off-main rejection, handler-thread routing, and
  position sanitization; they do not send actual media-key events.
- MPRIS backend tests exercise time conversion and metadata copying; they do
  not require a running D-Bus session. Capability gating is covered through
  `MprisCapabilities` / `PlatformConfig` unit tests, not a live bus.
- Windows SMTC behavior is untested because the backend is a stub.
