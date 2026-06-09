# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-06-09

### Added
- Added a Symphonia `AudioDecoder` for SACD DST packet streams.
- Added `DstDsdDecoder` for callers that need DST expanded to packed DSD bytes.
- Added PCM conversion through `rdsd2pcm` after DST-to-DSD expansion.
- Added registry helper support through `register_decoders`.

### Changed
- Exposes decoded Symphonia audio as planar `f32` PCM while preserving a lower
  level DSD byte-stream path.
