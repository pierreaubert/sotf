# sotf-ios (lib: `sotf_ios`)

iOS app shell for the SOTF music player. Compiles to a static library (`.a`) that the Xcode project links.

## Architecture

```
Swift AppDelegate -> sotf_ios_start() -> GPUI app callback -> PlayerView
Swift CADisplayLink -> gpui_ios_request_frame() -> GPUI render tick
```

The Swift side handles:
- Document picker for local file import
- MPNowPlayingInfoCenter for lock-screen metadata
- Music directory access

The Rust side handles:
- GPUI application lifecycle and rendering
- Audio engine and plugin chain
- Library scanning and playback logic

## Dependencies

- `gpui` / `gpui-ios` -- GPUI framework with iOS Metal backend
- `gpui-ui-kit` -- UI components
- `sotf-gpui` (app-gpui) -- Shared GPUI player application code
- `sotf-player` -- Player business logic
- `sotf-engine` -- Audio engine

## Building

Built as part of the Xcode project. The static library is linked into the iOS app target.

```bash
cargo check -p sotf-ios
```

## License

See the root workspace `LICENSE` file.
