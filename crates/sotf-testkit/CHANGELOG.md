# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-07-08

### Added
- Initial release of `sotf-testkit` as a shared workspace test-fixture crate.
- `audio` module with deterministic sine, log sweep, impulse, silence, white
  noise generators, WAV read/write, RMS/peak helpers, and centralized
  `data_tests/audio` lookups.
- `db` module with temporary SQLite database and temp-file helpers.
- Optional `engine` feature for virtual audio device detection and
  `EngineConfig` test helpers.
- Optional `plugin` feature with `SinglePluginFixture` and parameter round-trip
  helpers.
- Unit tests for `audio`, `db`, and `engine` helpers.
