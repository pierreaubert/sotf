# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-06-09

### Added
- Added the local SOTF workspace integration of `rdsd2pcm` for DSD-to-PCM
  conversion.
- Added reusable converter APIs used by Symphonia DSD/DST decoders and SACD
  extraction examples.
- Added configurable PCM output support for floating-point and integer WAV
  extraction paths.

### Changed
- Adapted conversion helpers for SACD-style packed DSD input and configurable
  output sample rates.
