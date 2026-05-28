# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-05-13

### Added
- Initial release of pure-Rust IAMF decoder.
- OBU (Open Bitstream Unit) parsing for IAMF v1.1.0 descriptors and temporal units.
- Codec support for Opus, AAC, FLAC, and PCM substreams.
- Ambisonics and speaker-layout rendering via `sotf-plugin-ambisonics`.
- Pre-allocated decode path with zero heap allocations in the hot loop.
