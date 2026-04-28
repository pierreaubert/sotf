# 0.5.115

- Canonicalized frequency-resolution choices so `ERB`, `Fine ERB`, and `Per Bin` map reliably to the analyzer modes `erb`, `fine_erb`, and `per_bin`.
- Reset per-band covariance, coherence, DOA, and decorrelation state when frequency resolution changes to avoid stale analysis history across band layouts.
- Smoothed high-latency and narrow-band analysis updates more conservatively to reduce covariance/coherence/DOA modulation artifacts in ERB, Fine ERB, and Per Bin modes.
- Smoothed per-band diffuseness before it modulates ambient gain and height suitability, reducing another block-rate analysis control path.
- Switched the main and HR FFT paths to sqrt-Hann WOLA analysis/synthesis so modified IFFT blocks are tapered at hop boundaries before overlap-add.
- Fixed `bypass_all_processing` so bypass passes stereo only to FL/FR and no longer synthesizes center-channel energy.
- Made `binaural_preview` a true 2-channel output mode, including HR-path fold-down and engine channel-flow reporting.
- Fixed app/GPUI channel accounting so toggling `binaural_preview` resizes the player graph, Matrix, meters, and workflow ports to stereo instead of retaining the previous surround layout.
- Added regressions for binaural-preview stereo output, bypass center silence, canonical frequency-resolution modes, and prime-sized host blocks across 5.0 through 9.1.6 layouts.
- Added `qa-upmixer isolate` to run controlled artifact-bisection variants on a track, report peak/step/hop-boundary/second-difference metrics, emit per-block diagnostics, optionally write comparison WAVs, and accept FLAC or other inputs through an `ffmpeg` QA fallback.
- Extended `qa-upmixer diagnose` with `--frequency-resolution` so ERB, Fine ERB, and Per Bin can be measured directly on the same input.
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
