# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Federation sources: Tidal/Spotify login flows (`l` = login, `L` = logout on a
  selected source, feature-gated `tidal`/`spotify`). Tidal shows the device-code
  prompt (URL + code + expiry) in a login panel while a background thread polls;
  Spotify opens the browser OAuth flow (URL shown as fallback) and caches
  librespot credentials. Tokens are persisted into the source config / credential
  cache and never displayed.
- The engine service-stream resolver is installed at startup, so the decoder
  thread resolves Tidal/Spotify streams itself (including gapless preload).

### Changed
- Playback no longer pre-resolves service streams in the UI thread; resolution
  happens in the engine decoder via the installed resolver hook.

### Fixed
- Tidal/Spotify login hardening: the Tidal device-code poll interval is now 5 s
  (matching GPUI and Tidal's `slow_down` guidance), a failed login thread spawn
  surfaces an error status instead of panicking, the Spotify logout arm is
  feature-gated like the Tidal one, and token-carrying login events redact
  tokens in their `Debug` output.

## [0.5.209] - 2026-07-08

### Added
- Status bar now shows source/output sample rates, a resampling (SRC) indicator,
  and a clipping/health warning from the read-only `SignalPath` model.
- Configure > Servers now includes a SOTF API section alongside MPD and DLNA,
  with editable enabled state, bind address, port, friendly name, auth token,
  and the URL remote SOTF apps should use.
- Enabling the SOTF API from the TUI generates an auth token when none is
  configured, matching server-mode startup behavior.
- `--server` startup now prints the active TUI log file path, and `--qa`
  applies before logging so QA runs write logs inside the selected QA directory.
- Added `--qr` to print a terminal QR code containing the SOTF API URL and
  bearer token for pairing remote SOTF apps.

### Changed
- TUI P1 hardening now keeps `App` under the struct-field budget, splits
  recording save state out of `RecordingTuiState`, sizes album-list truncation
  from the actual terminal width, clamps modal rectangles on tiny terminals,
  invalidates cached album-art image protocols on resize, and explicitly
  defers mouse support for this release.
- Server configuration navigation now cycles through API, MPD, and DLNA
  sections, and the screen opens on the SOTF API section so the remote-app
  connection details are immediately visible.
- Server mode prints compact SOTF API request lines to the terminal so remote
  client library/cache traffic can be inspected while the server is running.
- Output-device refresh and selection now tolerate output lists without a
  default device and avoid emitting a device-change command when nothing is
  selected.

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
