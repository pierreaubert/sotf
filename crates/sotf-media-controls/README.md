# sotf-media-controls

Cross-platform OS media controls (NowPlaying / MPRIS / SMTC) for SOTF apps.

In-house replacement for `souvlaki` with a tighter, dependency-light API:

- **macOS**: `MPRemoteCommandCenter` + `MPNowPlayingInfoCenter` via `objc2-media-player`.
- **Linux / FreeBSD**: MPRIS via `mpris-server`.
- **Windows / iOS / tvOS / other**: graceful no-op (`Error::Unsupported`).

The public surface mirrors the `souvlaki` slice we used, so migration is mostly mechanical.
