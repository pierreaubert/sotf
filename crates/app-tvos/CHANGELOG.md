# 0.5.2

## Changes

- AU plugins are working and I can load them but without a proper UI
- #140 apple TVOS is working on the simulator
- Fixed `crates/app-tvos/src/imp.rs` by closing a missing brace pair in `AssetSource` impl, resolving a tvOS parse/build failure.
