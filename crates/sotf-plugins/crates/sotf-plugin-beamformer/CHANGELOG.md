# 0.5.0

## New

- Added missing qa_*.rs files for some plugins
- Added examples: use pnd, mono2stereo and denoiser on old mono tracks
- Added an beamformer plugin

## Fixes

- **CRITICAL** Fixed STFT trigger bug: after first hop, `input_fill` reset caused ~257 FFT frames per 512-sample block instead of 2. Now correctly triggers every `hop` samples.
- **CRITICAL** Fixed missing overlap-add (OLA) in STFT synthesis path. Added `ola_buffer` with COLA-compliant sqrt(Hann) analysis/synthesis windows.
- **MAJOR** Fixed steering angle convention: docs said 0°=broadside but math implemented 0°=endfire. Rotated coordinate system so 0° is now actually broadside.
- **MAJOR** Fixed GSC Fixed Beamformer to use fractional delay compensation via per-mic delay lines instead of ignoring `steering_delays`.
- **MAJOR** Fixed GSC Blocking Matrix to match documentation (`B = I - d*d^H/(d^H*d)`) instead of adjacent-difference approximation.
- Did a round of test fixing

## Changes

- SOTA plugin improvements: shared DSP components + plugin upgrades
- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Massive update to plugins, see individual markdown plan for details
