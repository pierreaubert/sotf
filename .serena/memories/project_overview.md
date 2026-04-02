# SotF - Sound of the Future

## Purpose
SotF is a professional audio processing application that optimizes the sound of speakers and headphones.
It includes a full DSP plugin chain, audio engine, music player, and multiple frontends.

## Tech Stack
- **Language**: Rust (Edition 2024, stable toolchain)
- **Build**: Cargo workspace with ~80 crates, Just task runner
- **Frontends**: GPUI (native macOS GUI), TUI (terminal), CLI, iOS, tvOS
- **Audio**: Custom DAW-style plugin host, DSP plugins (EQ, compressor, limiter, crossfeed, upmixer, convolution, etc.)
- **Math**: Custom crates for IIR/FIR filters, DSP, optimization, room impulse response, Delaunay triangulation
- **Server**: MPD, DLNA, Chromecast, streaming, federation, TLS
- **Serialization**: serde (JSON, YAML), schemars
- **Async**: tokio
- **License**: GPL-3.0-or-later
- **Version**: 0.5.14

## Crate Organization
```
crates/
  app-gpui/        — GPUI desktop app (macOS)
  app-tui/         — Terminal UI app
  app-cli/         — CLI tools (recorder, etc.)
  app-ios/         — iOS app
  app-tvos/        — tvOS app
  sotf-engine/     — Audio engine (DAW host, graph processing)
  sotf-player/     — Shared player logic (business logic layer)
  sotf-plugins/    — Plugin framework + individual plugin crates
  sotf-types/      — Shared types
  sotf-midi/       — MIDI support
  sotf-tools/      — CLI utilities (audio test generation, SOFA conversion)
  sotf-server/     — Server crates (MPD, DLNA, Cast, streaming, federation, TLS)
  sotf-iamf/       — Immersive Audio Model and Formats
  math-audio/      — Math libraries (IIR/FIR, DSP, optimization, RIR, Delaunay)
  autoeq/          — Auto-EQ functionality
  gpui-toolkit/    — GPUI UI toolkit (themes, design system, iOS kit, builder)
  systemwide/      — System-wide audio (HAL driver, daemon)
  3rdparties/      — Vendored dependencies
  sotf-docs-gen/   — Documentation generator
```

## Architecture Pattern
- Business logic in `sotf-player`, app crates are thin UI wrappers
- Plugin host uses `Arc<Mutex<Box<dyn Plugin>>>` per node
- Audio processing is single-threaded (`process()` takes `&mut self`)
- Pre-allocated buffers to avoid per-frame allocations in audio callbacks
