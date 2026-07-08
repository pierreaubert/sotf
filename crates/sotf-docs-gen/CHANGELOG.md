# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.10] - 2026-07-08

### Changed
- Regenerated plugin reference pages (`crossover.md`, `xtc.md`) so the
  `just doc-crates` idempotence check passes.

## [0.6.8] - 2026-05-31

### Added
- Added `--check` and `--root` support for generated plugin docs.
- Added regression tests for markdown/YAML escaping, duplicate registry slugs, choice defaults, float formatting, idempotent writes, and CLI parsing.

### Changed
- Hardened plugin reference generation with escaped table/frontmatter values, finite float formatting, runtime registry validation, single-group headings, and no-parameter fallback text.
- Regenerated plugin reference pages from the updated generator.
- Marked `sotf-docs-gen` as unpublished and aligned package docs with its plugin-reference-only scope.

## [0.6.1] - 2025-05-13

### Added
- Added missing plugins in docs and AU/Clap/VST3 bridges.
- Updated documentation and added some missing plugins.

### Changed
- Automatic documentation for reference implementation.
