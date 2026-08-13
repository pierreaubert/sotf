# 0.7.10 (unreleased)

## Signal path visibility

- Footer and Plugin Graph now display source/output sample rates, a resampling (SRC) indicator, and an engine-health warning pill from the read-only `SignalPath` model.

## iOS / GPUI P1 hardening

- GPUI now applies iOS/tvOS left and right safe-area insets, uses an
  orientation-aware phone reference size for responsive scaling, and keeps
  `DensityMode::Expert` in compact layout on iPhone-sized windows.
- The iOS bridge now feeds Dynamic Type, memory-warning, and Low Power Mode
  events into the GPUI tick, and the Audio Devices settings surface includes a
  native AirPlay route picker on iOS.

## Plugin UI P1 hardening

- Added regression coverage that the Channel Mute/Solo plugin's M/S/D controls
  are wired to left-click handlers and mark plugin updates as structural.
- The EQ renderer now caches curve and per-band response data behind a keyed
  render cache instead of rebuilding response vectors inline every render.
- Upmixer config-tab metadata is now reused from static render metadata, and
  plugin UI tests guard the `Ds` design-token audit for fixed geometry.

## Remote SOTF connections and local-library cleanup

- Remote SOTF Players now live under Settings > Connections, accept API URLs
  with or without an explicit `http://` scheme, and require the SOTF API bearer
  token shown by the server instead of trying to connect to MPD credentials.
- Remote server API tokens are no longer only in memory: macOS stores them in
  Keychain, iOS uses the existing Swift Keychain bridge, and Linux/Windows use
  the internal remote-token store. Selecting a saved server reloads the token
  before starting the SSE event stream, fixing reconnects after app restart.
- Removing a remote server or clearing its token now deletes the persisted
  credential as well as the in-memory cache; `remote_servers.json` remains
  non-secret metadata only.
- The Local Library settings page is explicitly labelled local-only, scrolls on
  iPad-sized layouts, and includes a clear-local-library action for removing
  stale local album/track rows without deleting saved remote server records.
- iOS no longer shows the local-only Library or Keybindings settings tabs; when
  the connected remote server/library identity changes, the app clears stale
  local library rows plus the disposable remote album/artwork cache.
- Remote server tests in Connections now redraw when the probe finishes and
  time out promptly instead of leaving the row stuck on `Status: testing`.
- Connected remote players now show server-backed album pages in the Library
  screen, repaint when the background page cache completes, and send Library
  search text to the server-side paged album API instead of searching the
  cleared local database.
- Remote album cards now queue/play albums through the SOTF API, including
  touch-friendly Add and Play controls, and Home renders server-backed album
  shelves when connected to a remote player.
- GPUI wizard screens now keep their content panes height-constrained above the
  player footer, restoring vertical scrolling on iOS and other app targets.
- Connections settings now include a SOTF API card with a Show QR button that
  displays a scannable connection payload containing the API URL and bearer
  token.
- On iOS, Connections now includes a Scan QR button that opens the native camera
  scanner and adds the scanned SOTF API server through the same Keychain-backed
  remote connection path as manual entry.
- macOS direct and MAS packaging templates now include camera access entitlement
  plus `NSCameraUsageDescription` for QR-code server setup.

## SOTF API pairing and trust controls

- The Servers settings pairing toggle now calls the running authenticated SOTF
  API server to enable or disable pairing, so QR codes use the server's actual
  one-time nonce instead of local UI-only state.
- Pairing QR payloads now include a reachable LAN host instead of `127.0.0.1`,
  and trusted-client revocation goes through the server endpoint so the live
  mTLS verifier is updated immediately.
- The QR host helper now rejects loopback (`127.0.0.1`, `localhost`) and
  unspecified (`0.0.0.0`) bind addresses, ensuring the connection QR only
  advertises a concrete LAN interface.
- Added GPUI regression tests for QR host loopback/unspecified rejection and
  wildcard-to-loopback mapping for local API clients.
- Remote SSE handling now accepts both incremental server events and full
  `state` refresh frames from lag recovery or initial snapshots.

## Spatial spider visualizer

- New `components/plugins/spatial_spider/` module with `SpiderDisc2D` (2D horizontal disc) and `SpiderView3D` (two intersecting reference planes) elements. Both consume the polygon geometry built by `spatial_spider::data` and are themed via `SpiderColors::from_theme(&Theme)`.
- 2D disc shows concentric grid rings, radial rays every 30°, per-channel polygon (fill + stroke), speaker dots tinted toward `theme.error` for anti-phase correlation, channel labels, and a small centred LFE indicator.
- 3D view uses `d3rs::gpu3d::Lines3DElement` (new CPU-projected line/polygon renderer) with full mouse interactivity — left-drag rotates, middle-drag pans, scroll-wheel zooms via the embedded `OrbitControls`.
- Header bar exposes 2D/3D view toggle, SPL/Correlation mode toggle, and a reference-channel dropdown (Correlation mode only). The active reference channel is highlighted with a ring on its speaker dot.
- Single shared `render_spatial_spider_panel` helper backs both the upmixer's Spatial tab (custom view) and the generic `ui_layout_renderer` `"spatial_spider"` custom-viz hook so they stay in lockstep.
- AAE plugin opts into the spider via `VizSlot::Custom { name: viz_names::SPATIAL_SPIDER, position: VizPosition::FullCenter }` in its layout. Other multichannel plugins can opt in with the same one-line entry; `BelowGroup` positioning is honoured as well as `FullCenter`.
- Shared `SpatialSpiderUiState` on `AppState` survives tab toggles (view mode, ref-channel, orbit-camera state).

## Room EQ: new strategy options in dropdowns

- `MULTI_MEASUREMENT_STRATEGY_OPTIONS` now exposes `"spatial_robustness"`
  ("Spatial Robustness") and `"minimax_uncertainty"`
  ("Minimax (Bootstrap Uncertainty)"), in addition to the existing average
  / weighted-sum / minimax / variance-penalized choices.
- `MULTI_SEAT_STRATEGY_OPTIONS` now exposes `"continuous_area"`
  ("Continuous Listening Area"), alongside the existing variance / primary /
  average / modal-basis choices.
- All four strategy dropdowns (Multi-Measurement and Multi-Seat, in both
  the autoeq form and the room-eq configure step) automatically pick up the
  new entries through the existing form bindings; the UI carries the
  selection through to `sotf-player`'s `RoomEqOptimizerConfig`, which
  forwards it to `autoeq::roomeq`. Detailed sub-config editors (bootstrap
  num_resamples / α, area dimensions / bounds / seat positions, etc.) are
  not yet built — selecting these strategies currently uses library-side
  defaults until per-strategy panels are added.

# 0.6.5

## Plugin chassis themes — Graphite / Studio Cream / Brutalist

- New `components/plugins/theme/` module hierarchy — split from the
  former single `theme.rs` (renamed to `theme/meter.rs`, content
  unchanged).
- `PluginTheme` struct (in `theme/plugin_theme.rs`) defines a complete
  chassis-level visual language: chassis surfaces, panel backgrounds,
  ink scale, accent + arc + glow, LED indicators, font families
  (display / mono / UI), and dimensions (knob size, arc stroke,
  corner radii, section spacing). Independent from the global app
  `Theme` — chassis themes are a *replacement* layer, not a derivation.
- Three presets shipped:
  - **Graphite** (`theme/graphite.rs`) — default. Vintage psychoacoustic
    instrument: deep graphite chassis, warm amber calibration accents,
    Instrument Serif italic + Geist Mono. Inspired by Bruel & Kjaer field
    measurement gear.
  - **Studio Cream** (`theme/studio_cream.rs`) — light editorial: warm
    cream paper surfaces, terracotta tomato accent, Fraunces serif.
  - **Brutalist** (`theme/brutalist.rs`) — pure black / pure white,
    zero-radius corners, Archivo Black + IBM Plex Mono. Maximal
    contrast for projector use and accessibility.
- `PluginThemeId` enum with `all()` / `name()` / `next()` cycle helpers,
  plus serde `Serialize`/`Deserialize` for persistence.
- `RackThemeState { rack_theme, overrides: HashMap<usize, PluginThemeId> }`
  on `PluginState` — rack-level default cascades to every plugin without
  an override; per-plugin overrides take precedence. `set_override` /
  `clear_override` / `swap_overrides` / `on_plugin_removed` keep the
  override map aligned with the rack as plugins are added / reordered /
  removed.
- `PluginTheme::apply_to(&Theme) -> Theme` overlays the chassis palette
  onto a clone of the global theme — replaces surfaces, ink scale, accent,
  border, font_family; preserves semantic colors (error / warning /
  success / info / meter palette / plugin-type colors). This single
  adapter saves ~3000 lines of mechanical signature edits across the
  layout renderer and upmixer renderer; every existing helper that takes
  `&Theme` automatically picks up the chassis palette.
- `render_plugin_content` (in `components/plugins/mod.rs`) resolves the
  active `PluginTheme` once per render via the cascade, binds it locally,
  and passes `&PluginTheme` to both `ui_layout_renderer::render_from_layout`
  and `CustomViewRenderContext`. Loudness compensation and Upmixer
  (both routed through `render_from_layout`) repaint with the chassis
  palette; plugins with bespoke custom views (EQ, Spectrum, Matrix,
  Mute/Solo, MultibandComp/Exp, ABCompare, LoudnessMonitor) keep using
  the global theme — they receive `&PluginTheme` in the context but
  ignore it.
- `Config.rack_theme_state` (in `app/config.rs`) persists the rack default
  + overrides to user prefs (`#[serde(default)]` so old configs migrate
  cleanly to `PluginThemeId::Graphite`). Save/load wiring lives in
  `App::load_config_from` / `App::save_config_with_geometry`.
- Skin cycler button next to Load / Save in the rack header
  (`render_skin_cycler_button` in `components/plugins/ui_rack.rs`):
  click cycles the rack-level theme; **Shift + click while a plugin is
  being edited** cycles only that plugin's override. Label flips to
  `Skin: <name> ▸ #<idx>` to signal override mode. Cycling back to match
  the rack default automatically clears the override (so the map stays
  clean). Every click triggers `save_config` so the selection survives
  restart, mirroring the `cycle_theme` pattern.
- Cleanup hooks: `editing.rs::remove_plugin` calls `on_plugin_removed`
  *only* when the controller returns `PluginUpdateEffect::Structural`
  (so trying to remove a permanent plugin doesn't shift the override
  map); `move_plugin_up` / `move_plugin_down` call `swap_overrides`
  under the same guard.
- Defensive: skin cycler validates `editing_plugin_index` against the
  current plugin count before honoring it — `PluginController::remove_plugin`
  doesn't clear the field, so it can lag past a removal. A stale
  index falls through to the rack-default cycle path rather than writing
  a phantom override.
- 24 new unit tests across `theme/plugin_theme.rs`, `theme/graphite.rs`,
  `theme/studio_cream.rs`, `theme/brutalist.rs` — cover override cascade,
  swap / shift on remove, distinctness of preset accents, `apply_to`
  preserving semantic colors, and palette properties of each preset.

## Panel-divider drag: delta-based, no deadzone, no spurious clicks

- Replaced the absolute-position drag math in `ui/three_panel_layout.rs`
  (horizontal + vertical 3-panel modes) with delta-based dragging. Each
  divider's `on_drag_start` now records the mouse pixel position and the
  current ratio; `on_mouse_move` applies `(mouse - anchor) / denominator`
  to that recorded ratio. The ~100px deadzone before the divider would
  start tracking the cursor — caused by the prior code re-deriving the
  "current ratio" from the mouse alone, which disagreed with the
  solved-layout-clamped ratio — is gone.
- Added the corresponding `drag_anchor_pos` / `drag_anchor_*_ratio` fields
  to `LayoutState` (`app/state/ui.rs`). One pixel anchor is shared across
  dividers since only one can drag at a time; the per-divider ratio
  anchors keep the math correct under any starting state.
- `components/home/queue.rs` queue row `on_mouse_up` (left + right) now
  drops the click if any divider drag flag is set. GPUI bubbles mouse-up
  inner→outer, so the row handler runs *before* the layout-level handler
  resets the flags — the right place to catch and discard the trailing
  click that previously made finishing a divider drag over a queue row
  jump playback to a random track / pop a context menu.

## Meters panel: bars stretch with the panel

- `components/plugins/level_meters.rs::render_lufs_with_true_peak` now
  sets `.w_full()` on its outer `flex_col`. The bars (each `flex_1` inside
  `render_meter_bar`) had no width to share with the parent before, so
  they collapsed to intrinsic content size and left a wide empty band on
  the right when the meters panel was dragged wider. The same fix
  propagates automatically to the studio rack's Loudness Monitor plugin
  (`components/plugins/ui_loudness.rs:22`).
- `components/plugins/level_meters.rs::render_lufs_panel` drops the fixed
  `w(rems(25.0))` content cap; the LUFS / True Peak / Stereo Width
  section now fills the panel width.

## CLI: `--size WIDTHxHEIGHT` overrides saved window geometry

- `main.rs` adds a `--size` flag (e.g. `--size 1440x900`). The parser
  splits on `x`/`X`, validates positive finite floats, and rejects
  malformed values with a clear error before the GUI initializes. The
  override is applied to width/height after the saved preferences.json
  geometry is loaded; origin still comes from preferences so the window
  appears in the user's last-known position at the requested size.

## Queue: stale-index crash fixed + regression scenario

- `components/home/queue.rs:170` — queue row left-click and right-click
  handlers now bounds-check `queue_state.get(idx)` before indexing.
  Previously a captured `idx` from a render-time `enumerate()` could
  outlive a queue mutation (Clear Queue, magic-radio refill) and the
  next click would panic with `the len is 0 but the index is 0`,
  escalating to an FFI-frame abort because the panic crossed
  `gpui_macos::window::handle_view_event`.
- Added `dev_track("queue.row.{idx}")` on each queue row under
  `cfg(feature = "dev-api")` so `crates/sotf-dev-driver/scenarios/
  queue_stale_index.scn` can fire synthetic clicks against a known
  selector. Per the new project convention, every UI bug fix gets a
  `.scn` regression scenario alongside it; pure-logic bugs continue to
  go to unit tests.

# 0.5.19

## Room EQ recording: measured CTC matrix handoff

- Recording saves now export completed two-ear captures as measured
  CTC transfer-matrix IRs and include them in the saved RoomConfig.
- The Recording configuration screen now exposes CTC matrix strategy
  selection and loopback input. Raw-sweep mode persists the reference
  sweep, records a hidden loopback channel with each speaker/position
  take, and writes a `raw_sweep` CTC config for processing.
- Loading recordings into Room EQ preserves measured CTC data so the
  optimizer can solve against the in-room matrix instead of dropping it
  at the app boundary, including raw-sweep CTC configs.

## AutoEQ forms: wire Bayesian optimizer controls

- Added `autoeq:bo` to the shared AutoEQ optimizer selector and render
  BO controls for Sobol hot-start samples, batch size, posterior-std
  local-refiner handoff, acquisition (`qei` / `ei` / `thompson`), and
  qEHVI.
- Wired those controls into Room EQ, headphone EQ, and spinorama EQ
  state so the selected BO settings reach the optimizer. The Room EQ
  optimisation step now also summarizes and logs the BO parameters.

## Room EQ wizard: rename `target_tilt` → `target_response` (breaking)

- Step 3 ("Configure") and the AutoEQ form/render components now
  read and write the unified `target_response` field on
  `sotf_audio_player::RoomEqConfig` instead of the removed
  `target_tilt` / `broadband_target_matching` pair. This matches
  autoeq 0.4.30 and sotf-player 0.5.122.
- `app/types/room_eq.rs`, `components/autoeq/{config,form,render,
  render_body_room_eq,render_section_delay}.rs`, and
  `components/room_eq/step_{3_configure,4_optimise}.rs` were
  updated accordingly. The standalone broadband-matching render
  panel was removed — it is now a single checkbox under the
  target-response section.
- `tests/room_eq_config_tests.rs` was reshaped to drive the new
  field; tests that relied on legacy field names no longer
  compile against older sotf-player.

# 0.5.18

## Tests: enable `sotf-player/testing` feature in dev-dependencies

- Pulls `sotf-player` into `[dev-dependencies]` with the `testing`
  feature so `QueueController::add_album` skips the on-disk file
  check during integration tests. Without this, all 16
  `lifecycle::queue_sequences::*` and `lifecycle::playback_sequences::*`
  tests using synthetic `/test/*.flac` paths failed because
  `add_album` returned `Err` (silently discarded) and the queue
  stayed empty.
- Annotated all `queue.add_album(...)` and `app.add_album_to_queue()`
  test call sites with `let _ = ...` to silence the
  `unused_must_use` warnings now that the result is meaningful.

# 0.5.17

## Diagnostics

### Room EQ: multi-speaker regression tracing (no behavior change)

- Added diagnostic `log::info!` / `log::error!` entries in
  `step_4_optimise.rs::start_room_eq_optimization` around:
  - **pre-build**: `channel_measurements.len()`,
    `speaker_configs.len()`, and the `channel_names` /
    `speaker_config_names` lists.
  - **post-build**: `RoomConfig.speakers.keys()` and
    `RoomConfig.system.is_some()`.
  - **post-run**: `room_result.channel_results.keys()` vs
    `channel_names`, plus an `error!` when any expected channel is
    missing from `channel_results` — this is the UI-side silent-drop
    point identified while hunting the "second speaker never runs"
    regression reported against app-gpui roomeq.
- Purely additive; no behaviour changed. The logs surface the actual
  mismatch for user-side repro collection.

# 0.5.16

## Code changes

### Room EQ: dropped lossy DSP-chain conversion in Step-4 optimiser

- `step_4_optimise.rs` previously rebuilt the optimiser's `DspChainOutput`
  field-by-field into a stripped-down sotf-player copy, losing
  `initial_curve`, `final_curve`, `eq_response`, `target_curve`,
  `pre_ir`, `post_ir`, `loss_type`, `inter_channel_deviation`, and
  `epa_per_channel` on every run. Now passes
  `room_result.to_dsp_chain_output()` straight through — sotf-player's
  `DspChainOutput` is a re-export of the autoeq type, so no data loss.
- `room_eq_config_tests.rs` and `room_eq_apply_tests.rs` migrated to
  the rich `ChannelDspChain` / `DspChainOutput` shape via local
  `chain(...)` / `driver(...)` / `output(...)` helpers so the `None`
  soup of optional curve/IR fields doesn't pollute every test site.

## Fixes

### Room EQ: Schroeder split disabled for stereo without subwoofer

- `apply_smart_defaults` now only enables Schroeder split when a
  subwoofer is present. For 2.0 stereo, a single-pass optimizer across
  the full frequency range is more effective — the Schroeder split was
  fragmenting the optimization and preventing filters from landing on
  bass room modes.

### Room EQ: decomposed correction enabled

- `to_room_config()` now passes `DecomposedCorrectionSerdeConfig::default()`
  instead of `None`. This enables room mode detection and seeds the
  optimizer's initial guesses with detected modes at correct frequencies.

# 0.5.15

## Features

- Room EQ: `to_room_config()` now maps `"from_measurement"` tilt type
  to `TiltType::FromMeasurement`, enabling measurement-derived target
  tilt from the Simple Wizard.

# 0.5.14

## Fixes

### Room EQ "save to rack" — EQ filters were applied flat (all 0 dB)

- The `parse_filters` helper in `apply_room_eq_to_player` expected JSON
  keys `"frequency"` and `"gain_db"`, but the autoeq optimizer outputs
  `"freq"` and `"db_gain"`. Every filter silently fell through to the
  defaults (freq=1000 Hz, gain=0.0 dB), producing a flat EQ curve.
  The parser now accepts both key forms via `.or_else()` fallback.

### Room EQ "save to rack" — workflow graph canvas not refreshed

- The `WorkflowCanvas` entity was created once and never invalidated.
  When plugins were added or removed (from room EQ, spinorama EQ,
  headphone EQ, preset loading, or manual editing), the graph view
  kept showing the stale topology. Fixed by setting
  `workflow_canvas = None` on every structural plugin update so the
  canvas rebuilds on the next render.

### Room EQ "save to rack" — filter parser extracted for reuse

- The inline `parse_filters` closure in `apply_room_eq_to_player` was
  extracted to `sotf_audio_player::room_eq_types::parse_eq_filters_from_json`,
  making it testable and available to all frontends.

## Tests

- Added 9 integration tests for the save-to-rack flow (stereo, 5.1 surround,
  update existing EQ, no filters, missing channels, merged EQ plugins,
  non-EQ plugins skipped, case-insensitive plugin type, multi-driver
  rack incompatibility)

## Fixes

### Room EQ "save to rack" — silent failure on insert_plugin error

- `insert_plugin` returned `Result` but the error was discarded with
  `let _ =`. If graph insertion failed (e.g. non-linear topology), the
  code silently continued and tried to configure a plugin at the wrong
  index. Now uses `match` with proper error logging.

### Recording evaluation — magnitude plot was vertically flipped

- The "MAGNITUDE (dB)" chart in the recording evaluation screen
  (`components/recording/evaluating.rs::render_magnitude_chart`) was
  rendering every measured curve upside-down. A stray unary minus in
  the per-point normalization (`-(mag - normalization_offset)`) was
  flipping the sign of the offset-relative magnitude, so real room
  modes appeared as nulls and real cancellations appeared as peaks.
  The formula is now `mag - normalization_offset` and the chart
  matches both the raw `L.wav` / `R.wav` Welch PSD and the curves
  stored in `dsp.json` (which are also what `scripts/display-roomeq.py`
  has been displaying correctly all along).
- Phase, group-delay, distortion, RT60, clarity, impulse-response, and
  spectrogram charts were checked in the same pass and do *not* have
  the same bug — they use straight `mag - offset` or no normalization
  at all.
