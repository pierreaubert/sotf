# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.1] - 2026-07-08

### Added
- QA-SEC-006 negative abuse tests for oversized HTTP requests, HLS byte-range overflow, and unbounded segment allocation.

### Changed
- HLS media playlist parsing now enforces a `MAX_SEGMENTS` cap to prevent unbounded allocation from malicious playlists.
- `HttpMediaSource` reconnect now schedules exponential backoff without sleeping
  on the decoder read thread.
- MPD httpd stream startup now retries readiness with bounded short attempts
  instead of sleeping for a fixed 200 ms before connecting.

## [0.1.0] - 2025-05-13

### Added
- Initial release of HTTP streaming input for SOTF.
- `HttpMediaSource` — Symphonia-compatible HTTP media source with byte-range seeking.
- `IcyMetadata` — ICY metadata parsing for SHOUTcast/Icecast streams.
- Optional HLS support behind the `hls` feature.
