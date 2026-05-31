# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.7] - 2026-05-31

### Added
- Added regression tests for design-token hex parsing, Onyx import coverage, upmixer golden generation, WAV metadata writing, and SOFA float serialization.

### Changed
- Hardened design-token import errors and added CSS shorthand hex support.
- Updated generated audio-test SMPTE IMD metadata to report a 4:1 power ratio explicitly.
- Made upmixer golden generation reject non-block-aligned input instead of silently dropping tails.
- Switched `sofa-to-sqlite` to clap argument parsing and removed per-sample byte allocations.
- Pruned unused `sotf-tools` dependencies.

## [0.5.6] - 2025-05-13

### Added
- Initial release of `sotf-tools`.
- `generate-audio-tests` — Generate WAV test signals for end-to-end validation.
- `generate-upmixer-golden` — Generate golden-reference upmixer outputs.
- `sofa-to-sqlite` — Convert SOFA HRTF files to SQLite databases.
- `export-design-tokens` / `import-design-tokens` — Design token management.
