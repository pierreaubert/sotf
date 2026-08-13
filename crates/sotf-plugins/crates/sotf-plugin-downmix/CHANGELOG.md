# 0.5.28

## Fixes

- Require an explicit layout for ambiguous 8- and 10-channel inputs and carry
  layout identity through engine, factory, FFI, and Downmix Audio Unit setup.
- Preserve the published routing coefficients exactly; Downmix no longer
  silently rescales unrelated routes. Output headroom/limiting is an explicit
  downstream responsibility.
- Bound phase alignment to the ordinary downmix magnitude, confidence-gate
  unstable phase vectors, and remove per-bin trigonometry from the callback.
- Make realtime coefficient updates, cached snapshot updates, and reset reuse
  preallocated storage. Structural mode changes now require reconstruction.
- Validate independent N-to-2 FFI/AU channel widths and clear all streaming and
  filter state on reset or sample-rate reinitialization.

# 0.5.27

## Fixes

- Synchronized the engine's Downmix parameter accessor and settings/configuration
  wiring with the canonical nine-entry parameter schema, including `matrix_ltrt`.
  Added a regression test for host index round-tripping and serialized plugin
  configuration.

# 0.5.26

## Fixes

- Made phase-coherent WOLA output placement independent of host block partitioning and aligned the reported 2048-sample latency with the actual stream timeline.
- Replaced the magnitude-distorting Lt/Rt allpass-difference network with a unity-magnitude spectral quadrature rotation; Lt/Rt now uses the same fixed-latency WOLA path at every sample rate.
- Exposed Lt/Rt on the canonical parameter surface, marked topology-changing phase/LtRt modes structural, and reject incompatible simultaneous modes.
- Added fallible construction, finite/range/crossover validation, zero-sample-rate rejection, checked dimension arithmetic, and exact input/output buffer validation.
- Report conservative compile metadata for the signal-dependent/stateful downmixer and synchronize plugin/package versions.

# 0.5.25

## Fixes

- **STFT coefficient smoothing advances per FFT hop** — the phase-coherent path
  now advances coefficient smoothers as each FFT hop is processed instead of
  bulk-advancing once at the end of `process`. Added
  `test_stft_path_advances_coeff_smoothers_per_fft_block`.

# 0.5.24

## Fixes

- **LFE lookup state simplified** (`lib.rs`): replaced the per-channel
  `Vec<Option<usize>>` lookup with channel-indexed boolean flags and
  channel-indexed LFE filter state. This keeps the hot path O(1) without an
  unnecessary indirection and adds `test_lfe_lookup_uses_channel_indexed_flags`
  to pin the 5.1 LFE mapping.

# 0.5.23

## Fixes

- **WOLA COLA violation** (`lib.rs`): Changed `HOP_SIZE` from `FFT_SIZE/4` to `FFT_SIZE/2` and switched the analysis/synthesis window from full Hann to sqrt-Hann. Full Hann squared at 50% overlap is not COLA (`w²[i]+w²[i+N/2] = 0.75 + 0.25·cos(4πi/N)`), causing amplitude flutter. sqrt-Hann at 50% overlap satisfies COLA exactly (constant = 1.0). `output_scale` corrected to `1.0/FFT_SIZE` to match realfft's unnormalized IFFT.
- **Lt/Rt broadband 90° phase shift** (`lib.rs`): Replaced single first-order allpass (error up to ~88° at HF) with a 2-stage allpass chain (fc = [100, 132] Hz) minus a 1-sample delay approximation of a Hilbert transformer. The `process()` now returns `(chain_out, x_delayed)`; caller computes `shifted = chain_out - x_delayed`, giving ±31° accuracy from 200 Hz to 8 kHz.
- **Lt/Rt doc comment** (`lib.rs`): Fixed sign of Rs term: `Lt = L + 0.707·C - 0.707·j·Ls + 0.707·j·Rs` and `Rt = R + 0.707·C + 0.707·j·Ls - 0.707·j·Rs` (was swapped).

## Deferred

- **Issue #4 (Medium)**: Phase blend region uses a step function at the exact `(low+high)/2 Hz` boundary rather than a smooth fade across `[low, high]` Hz — deferred; requires redesign of per-bin blend logic.
- **Issues #5–9 (Medium/Low)**: Edge-case handling, parameter clamping, and minor refactors — deferred to next release cycle.

# 0.5.22

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin
- Fixed a lot of tests and then the corresponing code

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Plugins implemented f2,3 7,8,9,10,11,12 and 13 see product features for details
- Massive update to plugins, see individual markdown plan for details (wave 3)
- Massive update to plugins, see individual markdown plan for details (wave 2)
- Massive update to plugins, see individual markdown plan for details
