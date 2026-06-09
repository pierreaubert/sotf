# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-06-09

### Added
- Added a Symphonia `AudioDecoder` for packed DSD packet streams.
- Added registry helper support through `register_decoders`.
- Added PCM conversion through `rdsd2pcm` with default SACD/DSD64 output at
  176.4 kHz.

### Changed
- Exposes decoded audio as planar `f32` PCM through Symphonia audio buffers.
