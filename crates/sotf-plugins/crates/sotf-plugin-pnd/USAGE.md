# PND pitch-motion monitor

PND compares spectral partials between adjacent analysis frames and exposes a
relative pitch-motion estimate, confidence, matched-partial count, and total
peak count. Audio is returned sample-for-sample with zero reported latency.

This reference-free observation cannot identify absolute tuning or device-clock
error: stable 440 Hz and stable 444.4 Hz tones are both stationary and therefore
produce unity relative drift. Device correction requires source/render clock
timestamps; absolute pitch correction requires a pilot or note reference.

## Parameters

- `analysis_window_ms` (20–500 ms, structural): analysis-history duration.
- `drift_smoothing`: retained for state compatibility and future referenced
  correction; it does not alter monitoring audio.
- `multi_channel_analysis` (structural): confidence-weighted channel consensus.
- `confidence_threshold`: minimum confidence included in consensus.
- `correction_strength`: reserved at `0`; non-zero values are rejected.
- `phase_vocoder`: reserved at `false`; activation is rejected until a
  validated spectral shifter and latency contract exist.

## Realtime contract

After `initialize`, `process` and `reset` allocate no heap memory. Processing
accepts arbitrary callback partitions, returns exactly `context.num_frames`,
and copies every input sample to the corresponding output sample. Structural
parameter changes require graph reconstruction. A mismatched process sample
rate or malformed buffer is rejected transactionally.
