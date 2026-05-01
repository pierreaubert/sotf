# 0.13.0 (sotf vendored fork)

This is a vendored fork of upstream [coreaudio-rs](https://github.com/RustAudio/coreaudio-rs). Only changes specific to the SOTF workspace are tracked here; refer to upstream for the canonical history.

## Changes

- Patched the Apple-platform `cfg` gates so tvOS / watchOS / visionOS targets pick the same code paths as iOS (`cfg(target_os = "ios")` did not previously include tvos).
- Re-arranged into the workspace `crates/3rdparties/` tree so tvOS-Tier-3 patches stay isolated from upstream master.
