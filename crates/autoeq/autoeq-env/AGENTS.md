# autoeq-env (lib: `autoeq_env`, version: 0.4.0)

Shared environment utilities and constants for the AutoEQ subsystem.

## Purpose

Provides common environment configuration (paths, constants) used by the `autoeq` crate and its binaries.

## Key Exports

- `get_autoeq_dir()` -- Resolve `AUTOEQ_DIR` environment variable
- `get_data_generated_dir()` -- Path to generated data directory
- `get_records_dir()` -- Path to records directory
- `check_autoeq_env()` -- Validate environment setup
- `DATA_CACHED`, `DATA_GENERATED` -- Directory name constants
- `EnvError` -- Error type for environment issues

## Modules

- `env_utils.rs` -- Path resolution and environment variable handling
- `constants.rs` -- Directory name constants
- `log.rs` -- Logging macros

## Dependencies

Minimal: `thiserror`, `chrono`.

## Testing

```bash
cargo test -p autoeq-env --lib
cargo check -p autoeq-env && cargo clippy -p autoeq-env
```
