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
  not require a running D-Bus session.
- Windows SMTC behavior is untested because the backend is a stub.
