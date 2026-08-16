# sotf-plugin-pnd

PND is a fixed-frame, duration-preserving pitch-drift correction insert. Every
successful callback returns exactly `ProcessContext::num_frames`; device-clock
correction, FIFO fill control, timestamps, and variable-duration sample-rate
conversion belong at a stream boundary with independent producer and consumer
clocks. PND owns only its fixed-frame analysis/vocoder sample clock.

Correction uses a 2048-point, 512-hop Hann/WOLA phase vocoder with
instantaneous-frequency estimation, spectral-bin remapping, normalized
spectral-flux onset resets, and identity phase locking around remapped spectral
peaks. Its fixed causal latency is 2047 frames, including startup prefill and
group delay, independent of callback partitioning. An optional structural
`formant_preservation` mode estimates a smoothed log-magnitude envelope and
transports it to the original absolute frequencies with bounded gains;
`formant_strength` blends this correction from 0 to 1. The default mode remains
the legacy uniform correction path.

Automatic estimates compare adjacent analysis frames. Without an explicit
pilot, note, or clock reference they can detect change but cannot identify a
constant absolute pitch offset. Set `reference_frequency_hz` to a known pilot
or note for absolute correction; zero selects change-only tracking.

All channels are pitch-shifted independently with one shared correction ratio.
With multi-channel analysis enabled, low-confidence channels are excluded and
the remaining observations must form a confidence-weighted coherent cluster.
Contradictory channel estimates fail closed instead of being averaged into a
correction that no channel observed; silence and broadband noise do not outvote
a reliable tonal channel.

Legacy presets containing `phase_vocoder: false` or `true` migrate to the sole
duration-preserving engine. Schema v3 retains that migration and adds the
explicit formant mode while avoiding ambiguous fixed-frame SRC behavior.
