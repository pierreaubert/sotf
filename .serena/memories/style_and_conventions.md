# Code Style & Conventions

## Rust
- **Edition**: 2024
- **Toolchain**: Stable
- **Formatter**: `cargo fmt --all`
- **Linter**: `cargo clippy --all -- -D warnings`
- **clippy.toml**: `too-many-arguments-threshold = 100` (relaxed to allow complex audio plugin APIs)

## Naming
- Standard Rust conventions: snake_case for functions/variables, CamelCase for types/traits
- Plugin crates named `sotf-plugin-<name>` (e.g., `sotf-plugin-eq`, `sotf-plugin-crossfeed`)
- App crates prefixed with `app-` (e.g., `app-gpui`, `app-tui`, `app-cli`)

## Architecture Conventions
- Business logic belongs in `sotf-player`, never duplicated across GPUI and TUI
- Controllers consolidate shared logic (LibraryController, QueueController, PlaybackController, ScanController)
- Plugin parameters use `ParamSpec` system with `display_scale` for UI-scaled values
- No default/catch-all match arms — crash hard on unknown values
- Pre-allocate buffers; avoid allocations in audio callbacks

## Verification After Changes
1. `cargo check` (or with `--no-default-features` for plugins/engine crates)
2. `cargo clippy`
3. Run tests in the affected crate(s)
