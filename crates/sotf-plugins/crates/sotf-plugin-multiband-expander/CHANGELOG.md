# Changelog

## 0.5.23

### Bug fixes (critical / high)

- **COLA compliance** (`lib.rs:292`): Changed STFT overlap from 75 % to 50 %.
  Hann window is COLA-compliant at 50 % overlap; the previous 75 % setting
  produced a time-varying OLA gain (amplitude modulation at the hop rate ≈187 Hz).

- **Dry/wet mix latency compensation — spectral mode** (`lib.rs:1273`): The dry
  signal in spectral mode is now delayed by `fft_size - hop_size` samples (= 512
  at default FFT size) through a ring buffer before being mixed with the wet path.
  Previously the undelayed dry signal was mixed with the STFT-delayed wet signal,
  creating comb-filter notches at 1/latency spacing whenever `mix < 1.0`.

- **Dry/wet mix latency compensation — time-domain lookahead** (`lib.rs:2014`):
  Added `dry_lookahead_buffers` (one per channel) kept in sync with the per-band
  lookahead buffers. When `lookahead_ms > 0`, the dry path is now delayed by the
  same lookahead delay as the wet path, preventing comb-filtering.

- **Spectral magnitude normalization** (`lib.rs:1077`): Bin magnitude is now
  divided by the actual window DC sum (`window_sum = fft_size / 2` for Hann)
  instead of `fft_size / 2` incorrectly computed as `2 / fft_size`. This removes
  the ~6 dB under-estimation that caused the spectral-mode expander to trigger
  more aggressively than time-domain mode for the same settings.

- **Spectral latency over-reporting** (`lib.rs:1771`): `latency_samples()` now
  returns `fft_size - hop_size` (512) instead of `fft_size` (1024). Reporting the
  full FFT size caused the host to over-compensate by one hop, shifting the plugin
  output early relative to other tracks.

- **Spectral mode OOB panic with `num_bands > 5`** (`lib.rs:1009`): The
  fixed-size `[BandInfo; MAX_MB_BANDS]` array has been replaced with a `Vec<BandInfo>`
  sized to `num_bands`, eliminating the out-of-bounds index panic.

- **Measured auto-makeup stereo corruption** (`lib.rs:1968`): The makeup tracker
  is now updated once per frame using `max(envelope[L], envelope[R])` instead of
  once per channel, which was interleaving unrelated L/R envelopes into a single
  tracker and causing makeup gain jitter on stereo material.

- **`initialize()` missing `reset()` call** (`lib.rs:1733`): `initialize()` now
  calls `self.reset()` at the end. Previously, old envelope states, hold counters,
  and STFT ring buffers survived a sample-rate change, causing a transient click.

- **OLA ring clear size wrong** (`lib.rs:1150`): The pre-clear loop now clears
  `fft_size` positions (not `hop_size`). Clearing only `hop_size` left stale
  accumulation in the `fft_size - hop_size` un-cleared region, causing glitches
  on ring wrap-around.

### Bug fixes (medium)

- **Peak envelope follower release** (`lib.rs:1907`): The peak detector release
  is now a fixed fast 5 ms coefficient independent of the expander's attack time.
  Previously it used `attack_coeff`, so slow attack settings caused the peak
  envelope to lag, preventing the gate from closing promptly.

- **Hold-time truncation** (`lib.rs:1035, 1884`): Hold-time milliseconds-to-samples
  and milliseconds-to-hops conversions now use `.round()` before the `as usize`
  cast. Without rounding, values < 1 sample (e.g. 0.4 ms at 48 kHz) were silently
  truncated to 0.

- **`enable_ftz_daz()` missing in spectral path** (`lib.rs:1178`): Added at the
  top of `process_spectral_in_place`. FFT and complex-multiply loops generate
  denormals; without flushing them, CPUs without hardware DAZ can slow down 10–100×.

### Code cleanup

- Removed dead `latency_filled` field from `SpectralState` (was incremented but
  never read).
- Removed unused `RealFftPlanner` construction in `SpectralState::new` (the
  planner and its plans were created and immediately dropped).
- Removed `realfft` from `Cargo.toml` (no longer a direct dependency).

### Deferred

- **🔵 Dry/wet mix for time-domain without lookahead** — when `lookahead_ms == 0`
  there is no wet-path latency so no latency compensation is needed; already correct.
- **🟡 Spectral mode feature parity** (2.6): lookahead, `link_channels`,
  `detection_mode = rms`, sidechain HPF, and auto-makeup are not applied in
  spectral mode. These are cross-crate design gaps deferred for a dedicated feature.
- **🔵 SIMD per-sample log/pow** (4.7): performance advisory, no correctness impact.
- **🔵 `build_crossovers` mute/crossfade on `num_bands` change** (4.8): UX
  quality advisory, out of scope for this bug-fix pass.

## 0.5.22

- Added lookahead support (`lookahead_ms` parameter, 0–20 ms). Detection runs
  on the current sample while gain is applied to a delayed copy, letting the
  envelope "see" transients before they reach the output. Per-band, per-channel
  `LookaheadBuffer` circular delays, with latency correctly reported to the host.
- Added `lookahead_ms` to `GLOBAL_PARAMS` (index 17) and the multiband LAYOUT
  TIMING group.

## 0.5.21

- Initial multiband expander with LR4 crossovers, per-band dynamics,
  time-domain and spectral processing modes.
