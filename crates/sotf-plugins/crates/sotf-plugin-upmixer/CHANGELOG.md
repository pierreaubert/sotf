# 0.5.115

- Added `qa-upmixer diagnose` mode for block-by-block CSV diagnostics of output peaks, control deltas, dialogue detection, decorrelation strength, height gains, height flux gate, coherence, and per-channel levels.
- Added smoothed `dialogue_spatial_control` for spatial decomposition and panning so raw dialogue-probability jitter no longer directly modulates ambient gain, effective coherence, decorrelation strength, surround bleed, or height direct leak.
- Slew-limited the height flux gate and final height-band gain updates to reduce frame-to-frame mask chatter that can sound like grain or scratchiness.
- Initialized height mask state at the height floor to avoid startup/reinitialization jumps in height gain diagnostics.
- Extended diagnostics with `dialogue_spatial_control` and its delta for easier comparison against raw dialogue-probability movement.

# 0.5.114

- Corrected diffuseness/DOA analysis to use the full active-intensity vector, not just the real axis.
- Removed hot-path Vec allocations from smoothed LFE crossover table refresh.
- Moved input/output buffer validation before bypass processing so bypass returns clean errors instead of panicking.
- Marked FFT/decorrelation rebuilding controls as structural/setup so hosts don’t treat them as realtime automation targets.
- Added regressions for quadrature intensity classification and bypass buffer mismatch handling.
