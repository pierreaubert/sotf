# sotf-testkit

Shared test fixtures and helpers for the SOTF workspace.

## Overview

This crate is intended to be used only as a `dev-dependency`. It provides
deterministic audio signal generators, temporary database helpers, and
(behind optional feature flags) engine / plugin test harness helpers.

## Modules

- `audio` — Deterministic sine / sweep / impulse / silence / noise generators,
  WAV read/write, RMS/peak helpers, and centralized `data_tests/audio` lookups.
- `db` — Temporary SQLite database and temp-file helpers.
- `engine` (requires `engine` feature) — Virtual audio device detection
  (BlackHole / SotF HAL) and `EngineConfig` builders for integration tests.
- `plugin` (requires `plugin` feature) — `SinglePluginFixture` for driving a
  single in-place plugin, plus a parameter round-trip helper.
- `mock_server` (requires `plugin` feature) — Test-server helpers used by
  plugin integration tests.

## Features

- `default = []` — Only the core `audio` and `db` helpers.
- `engine` — Enables `sotf-engine` + `cpal` virtual-device helpers.
- `plugin` — Enables `sotf-host` plugin test fixtures.

## Usage

Add to a crate's `[dev-dependencies]` with the features it needs:

```toml
[dev-dependencies]
sotf-testkit = { path = "../sotf-testkit", features = ["engine", "plugin"] }
```

## Testing

```bash
# Core helpers
cargo test -p sotf-testkit

# Include optional engine / plugin helpers
cargo test -p sotf-testkit --all-features
cargo check -p sotf-testkit --all-features
```

Engine integration tests look for a virtual audio device. Install
[BlackHole](https://existential.audio/blackhole/) or set
`AEQ_E2E_DEVICE='Your Device Name'` to use a specific device.

## License

See the root workspace `LICENSE` file.
