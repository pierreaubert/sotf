# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-06-09

### Added
- Added a pure Rust SACD ISO reader and Symphonia `FormatReader`.
- Added SACD master TOC validation, stereo and multichannel area parsing, backup
  TOC fallback, track boundary calculation, and Scarlet Book sector handling.
- Added packet readers for uncompressed DSD and DST-compressed SACD tracks.
- Added Symphonia metadata population from SACD album, disc, and track text when
  available.
- Added `extract_sacd` example support for stereo, multichannel, all-track, and
  single-track extraction.
- Added DSF output for DSD and WAV output through the DSD/DST decoder path.

### Changed
- SACD DST packets can now be decoded through `symphonia-codec-dst` rather than
  being limited to preserved packet extraction.
