# sotf-media-controls

Cross-platform OS media controls (NowPlaying / MPRIS) for SOTF apps.

In-house replacement for `souvlaki` with a tighter, dependency-light API:

- **macOS**: `MPRemoteCommandCenter` + `MPNowPlayingInfoCenter` via `objc2-media-player`.
- **Linux / FreeBSD**: MPRIS via `mpris-server`.
- **Windows / iOS / tvOS / other**: graceful no-op (`Error::Unsupported`).

TODO(smtc): add a Windows System Media Transport Controls backend once SOTF has
a stable HWND/message-window ownership path.
