# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- `--gain-smoothing-ms` now defaults to 10 ms (the gain plugin param-spec default)
  instead of the previous 20 ms.

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
