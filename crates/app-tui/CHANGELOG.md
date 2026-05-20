# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.206] - 2025-05-13

### Added
- Room EQ pass-through support for new strategies: `minimax_uncertainty` and
  `continuous_area`. The TUI binding is string-based and goes through
  `to_optimizer_config()`, so the new strategies travel through unchanged.

## [0.5.205] - 2025-05-13

### Added
- Room EQ recording: N-by-M capture matrix. The recording channel list now
  expands across speakers, selected input mics, and measurement positions.
- Room EQ configuration now exposes CTC matrix strategy and loopback input
  fields. Raw-sweep mode writes the reference sweep and records a loopback WAV.
- Bayesian optimizer controls in Room EQ: `autoeq:bo`, BO hot-start samples,
  batch size, posterior-std stop threshold, acquisition, and qEHVI toggles.

### Changed
- Room EQ: renamed `target_tilt` → `target_response` (breaking). The TUI now
  consumes `TargetResponseUiConfig` in place of the removed
  `target_tilt` / `broadband_target_matching` pair.

## [0.5.204] - 2025-05-13

### Added
- Room EQ config builder now maps `"from_measurement"` tilt type to
  `TiltType::FromMeasurement`, enabling measurement-derived target tilt from
  the Simple Wizard.
