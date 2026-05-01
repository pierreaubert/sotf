# Quick Start - SOTF Audio Units

Get the SOTF Audio Unit suite running as macOS Audio Units (AUv3).

## Prerequisites

- macOS 15.0+
- Xcode 15+ with Command Line Tools
- Rust toolchain with both targets installed:
  ```bash
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  ```
- [`just`](https://github.com/casey/just): `cargo install just`
- [`xcodegen`](https://github.com/yonaskolb/XcodeGen): `brew install xcodegen`

## 5-Minute Setup

The build pipeline produces **two independent packages**, one per architecture, instead of a universal binary. Use the host-arch recipes for local development; use the explicit-arch recipes when you need both.

### 1. Build + package

```bash
just dist-au-arm64        # or dist-au-x86_64 for Intel
```

That recipe will:
1. Build `plugins-ffi` for the target arch and stage `libsotf_audio_plugins_ffi.a` into `Resources/`
2. Run `xcodegen` if `project.yml` is newer than the generated project
3. Run `xcodebuild` against the `SOTFAudioUnits` scheme with `ARCHS=<arch>`, isolating its `DerivedData` under `crates/sotf-plugins/crates/plugins-au/build/au-<arch>/`
4. Wrap the resulting `SOTFAudioUnits.app` into `dist/au/sotf-audio-units-<version>-macos-<arch>.pkg`

### 2. Install + register

Double-click the `.pkg` to install to `/Applications/`, then **launch `/Applications/SOTFAudioUnits.app` once** so macOS registers each `.appex` extension with the system.

### 3. Test

```bash
just validate-au-all      # Run auval against every SOTF subtype
just list-au              # Show registered SOTF AUs
```

Or open Logic Pro / GarageBand and look for **SOTF: …** under your audio effects.

## Build Both Arches (release flow)

For a release-style build that produces both arm64 and x86_64 packages:

```bash
just build-au-all                  # Build both per-arch packages
DEVELOPER_ID="Developer ID Application: Your Name (TEAMID)" \
  just sign-au                     # Sign both arches
APPLE_ID="you@example.com" \
  just sign-au-notarize            # Notarize, staple, and emit signed .pkg into dist/au/
just dist-au                       # Unsigned-or-Developer-ID-signed .pkg into dist/au/
```

Each step has per-arch variants if you want one architecture only:

| Action | Both | Per-arch |
|---|---|---|
| Build | `build-au-all` | `build-au-all-{arm64,x86_64}` |
| Sign | `sign-au` | `sign-au-{arm64,x86_64}` |
| Notarize | `sign-au-notarize` | `sign-au-notarize-{arm64,x86_64}` |
| Distribute | `dist-au` | `dist-au-{arm64,x86_64}` |

Build outputs land at:

```
crates/sotf-plugins/crates/plugins-au/build/au-arm64/Build/Products/Release/SOTFAudioUnits.app
crates/sotf-plugins/crates/plugins-au/build/au-x86_64/Build/Products/Release/SOTFAudioUnits.app
```

Distributable installer packages land at:

```
dist/au/sotf-audio-units-<version>-macos-arm64.pkg
dist/au/sotf-audio-units-<version>-macos-x86_64.pkg
```

## What You Get

- A SwiftUI/AUv3 wrapper around every plugin in `sotf-plugins`
- Per-arch packages ready to sign, notarize, and ship independently
- Direct linkage to `plugins-ffi` (the C FFI staticlib) — no inter-process overhead

## Troubleshooting

**Plugins not showing up in the DAW?**
```bash
ls -d /Applications/SOTFAudioUnits.app
just list-au
killall -9 AudioComponentRegistrar coreaudiod
open /Applications/SOTFAudioUnits.app   # launch once to register
```

**Build errors about missing libraries?**
```bash
# Confirm the staticlib was staged for the arch you're building
file crates/sotf-plugins/crates/plugins-au/Resources/libsotf_audio_plugins_ffi.a
# arm64 or x86_64 — should match the target you're building
```

**`xcodegen` complaining about `project.yml`?** Versions in `Info.plist`s and `project.yml` are kept in sync from `Cargo.toml` by the `sync-au-versions` recipe (called automatically before each build).

For deeper architecture and parameter-system docs, see `SETUP_GUIDE.md`.
