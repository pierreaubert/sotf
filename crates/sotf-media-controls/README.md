# sotf-media-controls

Cross-platform OS media controls (NowPlaying / MPRIS) for SOTF apps.

In-house replacement for `souvlaki` with a tighter, dependency-light API:

- **macOS**: `MPRemoteCommandCenter` + `MPNowPlayingInfoCenter` via `objc2-media-player`.
- **Linux / FreeBSD**: MPRIS via `mpris-server`.
- **Windows / iOS / tvOS / other**: graceful no-op (`Error::Unsupported`).

See [`PLATFORM_LIMITATIONS.md`](PLATFORM_LIMITATIONS.md) for the full matrix,
main-thread requirements, lifetime contract, and known testing gaps.

## Lifetime note

The closure passed to `MediaControls::attach` must be `Send + 'static`. It is
owned by the backend, and the backend joins its callback thread(s) and removes
OS-registered targets when dropped, so the closure cannot outlive app/player
state it captured. Pass an `Arc` or channel sender if the handler needs to
communicate with the rest of the app.

TODO(smtc): add a Windows System Media Transport Controls backend once SOTF has
a stable HWND/message-window ownership path.
