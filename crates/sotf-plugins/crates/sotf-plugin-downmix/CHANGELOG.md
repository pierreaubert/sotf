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
- Fixed WOLA window not satisfying COLA at 75% overlap: switched to sqrt(Hann) window with 50% overlap for perfect reconstruction, eliminating amplitude flutter on pure tones.
- Fixed Lt/Rt doc comment sign error for Rs channel.
- Replaced broken single first-order allpass (inaccurate beyond ~300 Hz) with a proper 2+2 stage parallel allpass phase-splitter for broadband 90° phase shift in Lt/Rt encoding.
- Added `phase_coherence_strength` parameter (default 0.5) to prevent full phase collapse of stereo width for out-of-phase content in the STFT path.
