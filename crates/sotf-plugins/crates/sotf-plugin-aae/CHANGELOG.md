# Unreleased

- Layout opts into the spatial-spider visualiser via `VizSlot::Custom { name: viz_names::SPATIAL_SPIDER, position: VizPosition::FullCenter }`. The app-gpui layout renderer picks this up automatically and appends the spider panel below the main control row.

# 0.5.3

- Content-aware dialogue ducking now uses windowed envelope evidence with a short hold, avoiding false ducking on quiet centered noise or steady mono-compatible music while staying active through sustained speech.
- LFE extraction now follows source-domain wet ER/FDN energy instead of signed routed speaker sums, so decorrelated channels cannot cancel the LFE send.
- The final rendered output now uses a linked safety limiter after auto-gain, preserving multichannel ratios while bounding summed dry, early, late, and LFE contributions.
- `ToneFilter`: clamp `a1` to `[-0.999, 0.999]` in both `new()` and `set_gains()`. Without the clamp, extreme gain ratios (e.g. `treble_ratio=0.2` + short RT60) place the feedback pole within ~0.001 of the unit circle, producing a reverberation tail that is effectively infinite (~millions of samples). Applies to both the `new` constructor and `set_gains` (fixes `src/tone_filter.rs`).
- `DelayLine`: fix `new(max_samples)` to allocate `max_samples + 2` slots internally so that `max_delay_samples()` returns exactly `max_samples`. Previously callers had to over-request by 1 sample to reach their target delay (fixes `src/delay_line.rs`).
- `EarlyReflections`: switch modulated tap reads from `read_linear` to `read_allpass`. Linear interpolation acts as a mild lowpass filter; allpass preserves the flat frequency response when delay lengths are time-varied. Each tap now maintains its own allpass state for continuity across calls (fixes `src/early_reflections.rs:100`).
- `EarlyReflections`: hard-code `MAX_PRESET_DELAY_MS = 154.5` to replace the O(presets × taps) scan inside `max_tap_delay_samples`. A `debug_assert` validates the constant against computed values during testing (fixes `src/early_reflections.rs`).
- `AaePlugin`: always pre-allocate `AutoGain` (even when `auto_gain_enabled=false`) in `from_params` and `initialize()`. This eliminates the heap allocation in `ensure_auto_gain()` that would otherwise occur on the audio thread the first time auto-gain is enabled via `set_parameter` (fixes `src/lib.rs`).
- `AaePlugin::process`: replace impossible runtime bounds checks (`tap_idx >= er_gains.len()`, `line_idx >= fdn_gains.len()`) with `debug_assert!` — these conditions are structurally impossible given construction invariants, but the checks were silently skipping taps in release builds (fixes `src/lib.rs`).

**Deferred (require cross-crate changes or larger refactors):**
- Issue #3 (`compute_lp_coeff` clamp at w=0.99): the function is only used at 120 Hz where the clamp never activates; the API confusion is noted but not fixed to avoid scope creep.
- Issue #4 (room-size click): smoothing the FDN delay-length transition requires a crossfade mechanism not yet present in the crate; deferred.
- Issue #8 (flat `Vec<Vec<f32>>` for VBAP matrices): correct direction but requires auditing all callers; deferred to a dedicated performance PR.
- Issue #10 (SIMD block processing): out of scope for a bug-fix release.
- Issue #6 (`signed_rms` polarity): function does not exist in this version of the code; review item was inapplicable.

# 0.5.2

- ER delay storage now provisions max preset delay plus max modulation headroom, uses fixed tap/state arrays, and supports preset changes without reallocating.
- Delay reads are clamped to capacity so out-of-range modulation cannot panic.
- FDN delay lines allocate for max room size up front; room_size updates now adjust delay lengths in place.
- Process buffer sizes now return clean errors, AAE reports zero host latency, content-aware dialogue ducking is implemented, ER stale tap summing is fixed, routing gain buffers are reused, and cached parameter updates avoid rebuilding the whole parameter list on every set.

# 0.5.1

Bug fixes

# 0.5.0

Initial version
