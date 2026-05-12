# Unreleased

## Fixes
- Fixed WOLA window not satisfying COLA at 75% overlap: switched to sqrt(Hann) window with 50% overlap for perfect reconstruction, eliminating amplitude flutter on pure tones.
- Fixed Lt/Rt doc comment sign error for Rs channel.
- Replaced broken single first-order allpass (inaccurate beyond ~300 Hz) with a proper 2+2 stage parallel allpass phase-splitter for broadband 90° phase shift in Lt/Rt encoding.
- Added `phase_coherence_strength` parameter (default 0.5) to prevent full phase collapse of stereo width for out-of-phase content in the STFT path.
