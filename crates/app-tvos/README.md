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

The tvOS app is built through the Xcode project in `tvos/`. The Rust static
library is produced by `tvos/build-rust.sh`, which is invoked automatically as
an Xcode pre-build script, so a clean checkout can build without manually
placing `libsotf_tvos.a` under `tvos/lib/`.

Prerequisites:

- Rust nightly toolchain with the `rust-src` component:
  ```bash
  rustup toolchain install nightly
  rustup component add rust-src --toolchain nightly
  ```

Generate the Xcode project and build for the tvOS simulator:

```bash
just tvos-sim
```

Or build the Rust library directly:

```bash
# Simulator
just tvos-rust-sim

# Device
just tvos-rust-device
```

`cargo check -p sotf-tvos` checks the host-compilable parts of the crate, but
the full app requires the tvOS Rust targets and Xcode.

## License

See the root workspace `LICENSE` file.
