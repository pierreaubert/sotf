# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.10] - 2026-07-08

### Added
- Add schema/version compatibility tests for persisted config/state types
  (`EngineConfig`, `PluginConfig`, `PluginGraphConfig`, `AudioEngineState`).
- Document stable vs internal fields in `SCHEMA.md`.

### Fixed
- Enable `serde_json/preserve_order` so JSON serialization of `serde_json::Value`
  preserves insertion order consistently across workspace and per-crate builds,
  eliminating non-deterministic snapshot drift in `snapshot_configs`.
- `AudioEngineState::latency_compensation_enabled` now uses the same default
  (`true`) when the field is missing during deserialization as it does in
  `Default::default`. Previously `#[serde(default)]` deserialized missing
  values as `false`.

## [0.6.8] - 2026-05-31

### Added
- Add `validate` and `try_new` invariant enforcement for engine, plugin, and plugin graph configs.
- Validate plugin graph node uniqueness, edge endpoints, positive input channel counts, and acyclicity.

### Fixed
- Reject invalid engine configs during load and save instead of silently accepting out-of-range values.
- Round buffer frame calculations up for fractional millisecond/sample-rate combinations.
- Migrate legacy engine config versions while rejecting configs from newer unsupported versions.

## [0.6.1] - 2025-05-13

### Changed
- Mode cleanup (-5k lines).
- Refactoring: extracted types from sotf crates to minimise dependencies.
