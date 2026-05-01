# sotf-media-controls

Cross-platform OS media-controls (NowPlaying / MPRIS / SMTC) for SOTF apps. In-house replacement for `souvlaki` with a tighter, dependency-light API.

## Architecture

- `lib.rs` — public API: `MediaControls`, `Error`, re-exports of `types`.
- `types.rs` — common types (`MediaControlEvent`, `MediaMetadata`, `MediaPlayback`, `MediaPosition`, `PlatformConfig`, `SeekDirection`).
- `backend.rs` / `backend/` — per-platform backends:
  - **macOS**: `MPRemoteCommandCenter` + `MPNowPlayingInfoCenter` via `objc2-media-player`.
  - **Linux / FreeBSD**: MPRIS via `mpris-server` (zbus-based).
  - **Windows / iOS / tvOS / other**: graceful no-op — `MediaControls::new` returns `Err(Error::Unsupported)`.

## Key Public API

- `MediaControls::new(config: &PlatformConfig)` — constructor.
- `MediaControls::set_metadata`, `set_playback`, `attach`, `detach`.

## Testing

```bash
cargo check -p sotf-media-controls && cargo clippy -p sotf-media-controls
cargo test -p sotf-media-controls
```

## Important Notes

- Public surface mirrors the `souvlaki` slice the rest of the workspace used; migration is mostly mechanical.
- Unsupported platforms must fail with `Error::Unsupported` rather than panic — callers fall back gracefully.
- macOS path uses modern `objc2-*` bindings; do not pull in `cocoa-rs`.
