# 0.5.123 (unreleased)

## Remote connection credentials and server-mode defaults

- Added a shared remote-token store for platforms without system Keychain
  support. Linux and Windows clients persist SOTF remote API bearer tokens in
  `remote_server_tokens.json`, keyed by `SotfRemoteServer::token_secret_key`,
  while `remote_servers.json` remains non-secret server metadata.
- Server mode now ensures the SOTF HTTP API is enabled and has an auth token
  when launched with `--server`, then persists those defaults so remote apps can
  connect to the API port without a separate manual bootstrap step.
- Added helpers for generating API auth tokens and formatting the advertised
  SOTF API URL for a configured bind address, matching the DLNA bind-address
  display behavior.

## Local library and audio-device handling

- Added shared helpers to clear persisted local library albums/tracks,
  playlists, FTS rows, and scan history while preserving source/connection
  records. GPUI uses this to remove stale local data when operating as a remote
  client.
- Expanded virtual/loopback device detection so SOTF virtual outputs, generic
  virtual devices, ZoomAudio, Background Music, Audio Bridge, and null outputs
  are skipped as automatic defaults, while still allowing explicit selection.

## Native SOTF remote pairing and SSE hardening

- SOTF API pairing now returns the command response shape expected by
  `SotfApiClient::complete_pairing`, consumes the pairing nonce after a
  successful registration, and updates the live MPD mTLS verifier trust set
  immediately when clients pair or are revoked.
- Pairing nonces are now 128-bit random values and are no longer exposed via
  public pairing status or LAN discovery TXT records. Discovery only advertises
  that pairing is available; the nonce stays in the authenticated enable
  response and QR flow.
- The API client now uses a no-timeout HTTP client for long-lived SSE streams
  and surfaces `event: state` frames as state snapshots instead of silently
  dropping them.

## Room EQ: surface uncertainty-aware and continuous-listening-area strategies

- `MultiMeasurementUiConfig` now carries an optional
  `bootstrap_uncertainty: BootstrapUncertaintyUiConfig` for the new
  `MultiMeasurementStrategy::MinimaxUncertainty` backend strategy. Fields:
  `num_resamples`, `alpha`, `seed`, `scalarisation` ("worst_case" | "cvar"),
  and `cvar_alpha` for the CVaR tail fraction.
- `MultiSeatConfig` (UI) now carries an optional
  `continuous_area: ContinuousListeningAreaUiConfig` for the new
  `MultiSeatStrategy::ContinuousArea` backend strategy. Flat string-based
  fields hold dimensions, axis-aligned bounds, calibration `seat_positions`,
  prior kind ("uniform" | "gaussian") with Gaussian mean/cov_diag/truncation,
  quadrature kind ("sobol" | "latin_hypercube" | "gauss_legendre") with the
  associated point counts and seed, scalarisation kind ("expected_value" |
  "worst_case" | "cvar") with worst-case inner-search budget and CVaR alpha,
  plus `idw_power` for the spatial measurement interpolator.
- `to_optimizer_config()` and `import_from_backend()` round-trip the new
  variants and sub-configs through `autoeq::roomeq`. Unknown strategy strings
  fall back to safe defaults at conversion time rather than panicking.

# 0.5.122

## Room EQ: export measured CTC transfer matrices from recordings

- Added a shared recording export helper that groups completed
  speaker × mic × head-position captures into measured CTC transfer
  matrices. It writes two-channel ear impulse-response WAVs under
  `ctc_matrix/` and returns `CtcMeasurementConfig` entries that roomeq
  can use as measured acoustic plants.
- Added an opt-in raw-sweep export strategy for the same matrix helper.
  It combines mic 0/1 raw recording WAVs into two-ear raw sweeps and
  attaches a per-take loopback WAV from a caller-selected input channel;
  the default remains the current impulse-response strategy.
- Recording device configuration now carries the selected CTC matrix export
  strategy and optional loopback input, while saved measurement metadata can
  preserve the full `CtcConfig` needed for raw-sweep processing.
- Legacy `RoomEqMeasurementsFile` metadata can now carry the measured
  CTC matrix alongside the normal channel measurements.

## Room EQ: graph playback for CTC and driver branches

- RoomEQ graph export now supports variable channel counts through global
  plugins such as downmix/upmix/XTC and keeps per-output correction branches
  at the full bus width.
- Multi-driver RoomEQ chains now expand into parallel graph branches, sum at
  a driver anchor, and then continue through the channel-level correction
  plugins. This keeps graph playback aligned with AutoEQ's joint CTC model.

## Room EQ: expose Bayesian optimizer settings upstream

- Added BO optimizer fields to the player-facing Room EQ config:
  `bo_initial_samples`, `bo_batch_size`,
  `bo_posterior_std_threshold`, `bo_acquisition`, and `bo_ehvi`.
  They now round-trip through `autoeq::roomeq::OptimizerConfig` so
  GPUI and TUI can drive the new `autoeq:bo` backend without each app
  knowing the backend schema.
- Added `RoomEqAlgorithm::BayesianOptimization` and carried the same
  BO knobs through headphone/spinorama optimizer configs and persisted
  AutoEQ argument state.

## Room EQ: follow autoeq 0.4.30 unified `target_response` (breaking)

- `RoomEqConfig` now carries a single `target_response:
  TargetResponseUiConfig` field in place of the legacy
  `target_tilt` + `broadband_target_matching` pair. The new struct
  mirrors `autoeq::roomeq::TargetResponseConfig` — target shape
  (`flat` / `harman` / `custom` / `file` / `from_measurement`),
  preference shelves (bass + treble), and the
  `broadband_precorrection` toggle.
- The serialized wire format changes accordingly. Saved user
  room-EQ configurations from 0.5.121 and earlier will not
  deserialize without manual migration because the two old fields
  no longer exist on the struct. This matches the autoeq config
  schema bump (1.3.0 → 2.0.0).
- `multi_speaker` no longer plumbs `target_tilt` through the
  optimizer configuration — callers are expected to set
  `target_response` directly.

# 0.5.121

## QueueController: gate file-existence validation behind `testing` feature

- `QueueController::add_album` and `play_album_now` now skip
  `validate_album_has_files` when the `testing` feature is enabled.
  Production builds keep the validation; integration tests in app-gpui
  (which use synthetic `/test/*.flac` paths) no longer have albums
  silently rejected with their `Result` discarded.
- Fixes 16 failing app-gpui lifecycle tests
  (`queue_sequences::*`, `playback_sequences::*`).

# 0.5.120

## EqFilterConfig dedup re-audit (no code change)

- The 0.5.118 CHANGELOG deferred unifying
  `room_eq_types::EqFilterConfig` with the canonical `PeqFilter`
  pending "a serde-alias migration". Re-audit shows
  `EqFilterConfig` is **not** a stripped autoeq record — it is the
  runtime/UI-side canonical matching `sotf-engine::EQFilter`
  (`filter_type` / `frequency` / `q` / `gain_db`), which is the
  wire format the engine plugin loader deserializes. `PeqFilter`
  and its aliases (`HeadphoneEqBiquad`, `SpinoramaBiquad`) use the
  autoeq-side convention (`filter_type` / `freq` / `q` / `db_gain`).
  The two conventions exist by design and the codebase already has
  an explicit bridge (see `app-tui::events::conf_roomeq` mapping
  `b.freq → frequency` and `b.db_gain → gain_db`).
- Adding `#[serde(alias = "frequency")]` + `#[serde(alias = "gain_db")]`
  to `PeqFilter` would make deserialization tolerant but
  serialization would still emit `freq` / `db_gain`, silently
  breaking any consumer (including `sotf-engine::EQFilter`) that
  parses JSON by field name. `#[serde(rename)]` flips the output
  key — equally breaking. There is no painless serde-only
  unification, so the two structs stay separate. Any future
  consolidation needs a coordinated change across both wire formats
  plus a deprecation window, not a quiet alias.

# 0.5.119

## Bug fixes

- `SpeakerOptimizationResult`: the seven CEA2034 spinorama curves
  (`on_axis_curve`, `lw_curve`, `er_curve`, `sp_curve`, `pir_curve`,
  `er_di_curve`, `sp_di_curve`) are now populated with empty
  `Vec<f64>` when spin data is absent, instead of zero-filled vectors
  sized to `frequencies.len()`. Consumers in `speaker_graphs.rs`
  (e.g. `render_spinorama_main_response_plot`,
  `render_tonal_balance_plot`) and `SpinoramaCurves::is_valid` /
  `has_pir` already use `is_empty()` as an absence sentinel — the
  previous zero-filled vectors silently passed those checks and
  caused misleading flat-line plots at 0 dB. Affects both the
  single-speaker `From<SpeakerOptResult>` conversion and the
  multi-speaker `to_speaker_results` builder.

## Tier-1 dedup re-audit (no code change)

- Re-audit of `sotf-player::room_eq_types` against
  `autoeq::roomeq::types::config::*` found the six candidates
  (`TargetTiltConfig`, `ExcursionProtectionConfig`,
  `SchroederSplitConfig`, `PhaseAlignmentConfig`, `MultiSeatConfig`,
  `BroadbandTargetMatchingConfig`) are **not** trivial stripped
  copies: each pair diverges in at least one of default values,
  field names (`slope` vs `slope_db_per_octave`), field types
  (`String` vs enum, `f64` vs `Option<f64>`), field nesting
  (flat vs nested `LowFreqFilterConfig` / `HighFreqFilterConfig`),
  or field count (extra `enabled` flag on the UI side that the
  backend represents as `Option<T>`). They stay separate by design;
  any future unification needs a serde-alias migration plus
  behavioural review, not a blind `pub use`.

# 0.5.118

## Code changes

- Collapsed the two identical stripped biquad records
  (`HeadphoneEqBiquad` and `SpinoramaBiquad`) into a single canonical
  `PeqFilter` struct in the new `peq_filter` module. Both names remain
  exported as type aliases, so all GPUI/TUI call sites keep compiling
  unchanged. The duplicated 4-field struct definition with identical
  derives and identical serde representation is gone.
- `room_eq_types::EqFilterConfig` is **not** aliased here because it
  uses the `frequency` / `gain_db` naming convention instead of the
  autoeq-shaped `freq` / `db_gain` pair, so unifying it requires a
  separate serde-alias migration.

# 0.5.117

## Features

- Simple Wizard: all speaker tiers (NearField, MidField, FarField) now
  use `from_measurement` target tilt instead of hardcoded slopes. The
  optimizer derives the target slope from the measurement curve at
  optimization time, preserving the speaker's natural response.
- `import_from_backend()` handles the new `TiltType::FromMeasurement`
  variant.

# 0.5.116

## Code changes

- Extracted `parse_eq_filters_from_json` from inline closure to a public function in `room_eq_types` — now testable and reusable across all frontends

## Tests

- Added 9 unit tests for `parse_eq_filters_from_json` (autoeq/engine JSON key formats, all filter types, defaults, edge cases)
- Added 4 unit tests for `DspChainOutput::is_rack_compatible` (no drivers, with drivers, mixed, empty)
- Added 6 unit tests for save-to-rack plugin graph operations (insert EQ, update existing, per-channel config serialization, disabled EQ exclusion)

Bug fixed:
- added check when the album art or the music files are not currently available (classical example is music is on an external drive currently not mounted)
- forced all paths to be loaded through cleanup routines (affect plugins loading)
- preset filename traversal now extract only the filename component
- replaced unwrap() on map lookups with expect() containing a clear diagnostic message
- improved migration error message from "Unknown" to "Unsupported ... (minimum: N)".
- fixed apply to rack and apply to graph hosts (lots of changes there)
