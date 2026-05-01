# 0.6.1

Swift / Xcode-driven AUv3 bridge for the SOTF plugins. Not a Rust crate; built via the `Justfile` recipes.

## New

- Added the missing newer plugins (and AAE — experimental) to the AU bridge so the AUv3 `.appex` bundle exposes the full SOTF plugin set.
- Added a directory-per-plugin layout under `plugins-au/` so each AUv3 target lives in its own folder.
- New `dist-au-arm64` / `dist-au-x86_64` / `dist-au` recipes produce a notarisation-ready `.pkg` installer in `dist/au/`.

## Fixes

- macOS: forced every AUv3 target to share the same version number as the parent bundle so the system AU registry stops rejecting mixed versions.
- Several rounds of test fixes + snapshot updates after adding the missing AU plugins.

## Changes

- Replaced the old `install-au-all` flow with the `.pkg` install path so testing in DAWs goes through the packaged installer (matches the release path).
- Iterative work on the AU UI-generation pipeline (still not feature-complete; tracks the upstream automatic UI generator for plugins).
