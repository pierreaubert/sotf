# sotf-tvos (lib: `sotf_tvos`)

tvOS (Apple TV) app shell for the SOTF music player. Compiles to a static library (`.a`) that the Xcode project links.

## Architecture

```
Swift AppDelegate -> sotf_tvos_start() -> GPUI app callback -> PlayerView
Swift CADisplayLink -> gpui_ios_request_current_frame() -> GPUI render tick
```

Key differences from the iOS app (`sotf-ios`):
- No document picker (no local file import)
- No MPNowPlayingInfoCenter (tvOS doesn't support lock-screen metadata)
- Input via Siri Remote (focus engine + button presses) instead of touch
- Larger default font scale for 10-foot viewing distance

## Dependencies

- `gpui` / `gpui-ios` -- GPUI framework with iOS/tvOS Metal backend
- `gpui-ui-kit` -- UI components
- `sotf-gpui` (app-gpui) -- Shared GPUI player application code
- `sotf-player` -- Player business logic
- `sotf-engine` -- Audio engine

## Building

Built as part of the Xcode project. The static library is linked into the tvOS app target.

```bash
cargo check -p sotf-tvos
```

## License

See the root workspace `LICENSE` file.
