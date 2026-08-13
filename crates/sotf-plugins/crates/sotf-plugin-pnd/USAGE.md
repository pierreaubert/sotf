# PND pitch-drift correction

PND is a duration-preserving insert for tonal programme material. It always
consumes and produces the host callback's frame count. It is not a device-clock
synchronizer and does not perform variable-duration SRC.

Parameters:

- `correction_strength`: 0–200%; 0 monitors, 100% applies the estimated correction.
- `analysis_window_ms`: 20–500 ms detector history; structural/setup-only.
- `drift_smoothing`: 1–1000 ms sample-clock time constant.
- `multi_channel_analysis`: analyze all channels using confidence-weighted consensus; structural/setup-only.
- `confidence_threshold`: minimum detector confidence that authorizes correction.
- `reference_frequency_hz`: known pilot/note for absolute correction; 0 uses change-only tracking.

Signal flow:

```text
fixed-frame input
  -> per-channel FFT peak analysis
  -> referenced or change-only drift estimate
  -> confidence gate and sample-clock smoothing
  -> shared-ratio, per-channel phase-vocoder correction
  -> fixed-frame output (2047-frame latency)
```

The phase vocoder remaps spectral energy and instantaneous frequency. It has no
formant-envelope model and no identity/peak phase locking, so large corrections
or strongly percussive material can smear transients. Prefer small, slowly
varying corrections and monitor confidence.

Legacy `phase_vocoder` values are accepted only for preset compatibility. Both
`false` and `true` migrate to the duration-preserving engine and the field is
omitted from new state.
