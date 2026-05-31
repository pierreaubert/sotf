# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
