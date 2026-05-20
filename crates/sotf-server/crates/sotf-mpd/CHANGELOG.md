# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.1] - 2025-05-13

### Added
- Initial release of MPD protocol server for SOTF.
- TCP server accepting MPD protocol connections (`MpdServer`).
- Command parsing and response formatting (`MpdCommand`, `MpdResponse`).
- Optional TLS support behind the `tls` feature.
