# 0.5.4 (unreleased)

## Build integration and versioning (QA-TVOS-001)

- Adopted the same derived Rust static-library build pattern as iOS:
  - Added `tvos/build-rust.sh`, invoked as an Xcode pre-build script, which
    builds `libsotf_tvos.a` into Xcode's `DERIVED_FILE_DIR/rust/`.
  - Updated `tvos/project.yml` to link against the derived archive instead of
    `$(PROJECT_DIR)/lib/libsotf_tvos.a`.
  - Added `tvos/.gitignore` so the `lib/` directory is no longer expected to be
    checked in.
  - Updated `builds/tvos.just` to remove the manual `tvos/lib/` copy steps and
    documented that the Xcode pre-build script handles the Rust library.
- Added `MARKETING_VERSION` and `CURRENT_PROJECT_VERSION` (0.6.8) to
  `tvos/project.yml` and wired them into `SotFTV/Info.plist` via Xcode build
  variables.
- Added host-side tests verifying the derived build pattern, version metadata,
  and required shipping assets.
- Updated `README.md` with nightly + rust-src prerequisites and the `just tvos-sim`
  build instructions.

## Security hardening

- Added a tvOS `PrivacyInfo.xcprivacy` manifest declaring accessed API reasons
  for file timestamps, disk space, and user defaults, with no tracking or
  collected data types.
- Added host-side Rust tests that keep the tvOS privacy manifest present and
  included through the XcodeGen source folder.

# 0.5.2

## Changes

- AU plugins are working and I can load them but without a proper UI
- #140 apple TVOS is working on the simulator
- Fixed `crates/app-tvos/src/imp.rs` by closing a missing brace pair in `AssetSource` impl, resolving a tvOS parse/build failure.
