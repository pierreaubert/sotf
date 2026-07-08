# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.1] - 2026-07-08

### Added
- QA-SEC-006 negative abuse tests for malformed Tidal control JSON, invalid Spotify credential types, and secret-free error messages.

## [0.1.0] - 2025-05-13

### Added
- Initial release of streaming service integrations for SOTF.
- `StreamingService` trait for common backend interface.
- Optional Spotify Connect support via librespot (`spotify` feature).
- Optional Tidal API integration via reqwest (`tidal` feature).
