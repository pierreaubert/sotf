# sotf-tvos (lib: `sotf_tvos`)

tvOS (Apple TV) app shell -- static library (.a) linked by the Xcode project.

## Architecture

```
Swift AppDelegate -> sotf_tvos_start() -> GPUI app -> PlayerView
Swift CADisplayLink -> gpui_ios_request_current_frame() -> GPUI render tick
```

Key differences from iOS (`sotf-ios`):
- No document picker (no local file import)
- No MPNowPlayingInfoCenter
- Siri Remote input (focus engine + button presses) instead of touch
- Larger default font scale for 10-foot viewing

## Dependencies

- `gpui` / `gpui-ios` -- GPUI framework with tvOS Metal backend
- `sotf-gpui` (app-gpui) -- Shared GPUI player code
- `sotf-player` / `sotf-engine` -- Player logic and audio engine

## Testing

```bash
cargo check -p sotf-tvos
```

## Important Notes

- Compiles as `staticlib` crate type
- Uses `extern "C"` FFI boundary with Swift
- Shares all UI code with `app-gpui`
