# autoeq-env (lib: `autoeq_env`, version: 0.3.0)

Shared environment utilities and constants for the AutoEQ subsystem.

## Purpose

Provides common environment configuration (paths, constants) used across `autoeq`, `autoeq-cea2034`, and related crates.

## Dependencies

Minimal: `thiserror`, `chrono`.

## Testing

```bash
cargo test -p autoeq-env --lib
cargo check -p autoeq-env && cargo clippy -p autoeq-env
```
