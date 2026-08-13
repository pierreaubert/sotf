# 0.5.7

- Replace the waveform-cycle binary gate with fast/slow power tracking,
  persistence, hysteresis/hold, continuous reduction depth, and sample-rate
  derived gain timing.
- Smooth live cutoff and bypass transitions while keeping detector/filter state
  warm; zero strength and settled bypass remain exactly transparent.
- Canonicalize the visible cutoff against the active sample rate, reject invalid
  topology/rates and unknown preset fields, sanitize non-finite audio, and snap
  decaying state out of the denormal range.
- Make realtime parameter setters allocation-free, use the crate version in host
  metadata, and expand deterministic DSP, metadata, factory, and QA coverage.

# 0.5.6

- **Fix:** require nonzero initialization and reject processing at an uninitialized or mismatched sample rate.
- **Fix:** canonicalize persisted and runtime parameters against the documented ranges.
- **Fix:** use sample-rate-derived envelope/gain smoothing and a fast/slow envelope detector to avoid per-sample modulation clicks.
- **Performance:** classify the plugin as IIR and reuse the cached parameter schema instead of rebuilding it on every query.

# 0.5.5

- **Fix:** disabled/bypassed processing now validates host buffer size before returning. The plugin
  no longer accepts malformed buffers only because hiss reduction is bypassed.
- **Docs:** clarified that latency reporting is delegated to the underlying IIR reducer and correctly
  reports zero algorithmic latency.

# 0.5.5

- **Fix (critical): removed dead `low_latency` parameter** (`src/params.rs`, `src/lib.rs`). The
  underlying `HissReducer` is a simple first-order IIR filter with no FFT at all; exposing a
  "Low Latency (smaller FFT)" toggle was pure dead UI surface that silently did nothing. The
  parameter is removed from `PARAMS`, `LAYOUT`, `HissReducerPluginParams`, and `params::Params`.
  Serialised presets that include `low_latency` will silently drop the field via serde's default
  handling — no migration needed.
- **Fix (high): parameter changes no longer reset DSP state** (`src/lib.rs:set_parameter`).
  Previously every change to `threshold_db`, `frequency_hz`, or `strength` called
  `rebuild_reducer()`, which re-created the `HissReducer` from scratch and reset all IIR history
  and envelope-follower state, causing audible clicks. The fix calls
  `reducer.set_params(frequency_hz, threshold_db, strength)` in-place instead; internal
  coefficients are updated without touching filter state. `rebuild_reducer()` is now removed.
- **Fix (medium): initial sample-rate mismatch** (`src/lib.rs:from_params`). The plugin stored
  `sample_rate: 44100` before `initialize()` was called, but `HissReducer::new()` internally
  defaults to 48000 Hz. Changing it later in `initialize(44100)` would silently re-derive filter
  coefficients for a different rate, altering the frequency response. The stored default is now
  48000 to match the reducer's construction-time default. Callers must still call `initialize()`
  with the actual host sample rate — this fix only removes the inconsistency before that call.
- **Fix (medium): `latency_samples` reporting** (review issue 4). The review assumed FFT-based
  processing. `HissReducer` is a sample-by-sample IIR lowpass with no look-ahead or buffering;
  the plugin now delegates to `HissReducer::latency_samples()`, which correctly returns `0`.

# 0.5.4

- Initial release. Split out of `sotf-plugin-denoiser` into a dedicated stationary high-frequency hiss reducer.
- Uses the shared `HissReducer` core from `plugins-denoiser`.
- Parameters: `enabled`, `threshold_db` (SNR threshold), `frequency_hz` (cutoff above which hiss removal applies), `strength` (0.0–1.0), `low_latency` (smaller FFT path).
