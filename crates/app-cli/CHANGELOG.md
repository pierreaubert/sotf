# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- Initialize the new PND reference-frequency field explicitly in CLI-created
  plugin settings; zero preserves change-only compatibility behavior.

## [0.5.22] - 2026-07-08

### Added

- Added `diagnostics` subcommand (`player-cli diagnostics`) that exports a
  secret-safe JSON diagnostics bundle built by `sotf-player`.
- Added `--why-no-audio` flag to the `diagnostics` subcommand for a compact,
  actionable explanation of common playback-failure causes.
- Added `error_output` helper that redacts URLs and secret-bearing query
  parameters (`token=...`, `api_key=...`, `password=...`, etc.) from CLI error
  output and media-path log lines before printing or logging.
- Added unit tests for secret redaction and integration tests for stable help
  output and safe error formatting.

### Changed

- `--gain-smoothing-ms` now defaults to 10 ms (the gain plugin param-spec default)
  instead of the previous 20 ms.
- Fixed `README.md` and `AGENTS.md` examples so they use the current subcommand
  and argument names (`play`, `--upmixer-config`, `--crossfeed-preset`,
  `--hwaudio-send-to`, `--hwaudio-record-from`).

## [0.5.21] - 2025-05-13

### Added
- Added the new plugins to players.
- Added playlist support across the board.

### Fixed
- Fixed bug with federation.

### Changed
- Splitted the denoiser and adapted the ecosystem on top.
- Move presets from apps to autoeq.
- SOTA plugin improvements: shared DSP components + plugin upgrades.
- Next iteration on UI and testing for plugins this time with native look & feel.
- First step of automatic UI generation via a set of constraints; non-regression is built in with insta.
- Another round of parameters update.
