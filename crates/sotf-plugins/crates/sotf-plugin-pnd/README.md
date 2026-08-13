# sotf-plugin-pnd

Reference-free polyphonic pitch-motion monitoring with exact, allocation-free
audio passthrough. The analyzer reports inter-frame drift, confidence, and
matched spectral partials; it does not infer absolute pitch or device clock
error without a pilot, timestamp, or independent clock reference.

The legacy `correction_strength` and `phase_vocoder` keys remain readable for
preset compatibility, but non-zero correction and phase-vocoder activation are
rejected. Variable-duration resampling cannot safely implement persistent
correction inside a fixed-frame insert, and the retired phase-vocoder path did
not perform validated spectral-bin remapping.
