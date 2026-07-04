# sotf-ios (lib: `sotf_ios`)

iOS app shell -- static library (.a) linked by the Xcode project.

## Architecture

```
Swift AppDelegate -> sotf_ios_start() -> GPUI app -> PlayerView
Swift CADisplayLink -> gpui_ios_request_current_frame() -> GPUI render tick
```

- Swift handles: document picker, MPNowPlayingInfoCenter, music directory access
- Rust handles: GPUI app lifecycle, audio engine, library scanning, playback

## Dependencies

- `gpui` / `gpui-ios` -- GPUI framework with iOS Metal backend
- `sotf-gpui` (app-gpui) -- Shared GPUI player code
- `sotf-player` / `sotf-engine` -- Player logic and audio engine

## Testing

```bash
cargo check -p sotf-ios
```

## Important Notes

- Compiles as `staticlib` crate type
- Uses `extern "C"` FFI boundary with Swift
- Shares all UI code with `app-gpui`
