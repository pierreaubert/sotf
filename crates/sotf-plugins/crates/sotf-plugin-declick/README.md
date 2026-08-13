# sotf-plugin-declick

SOTF Declick detects short impulsive corruption from robust context and
reconstructs it by interpolation. It uses eight samples of fixed lookahead
(0.167 ms at 48 kHz), reports that latency to the host, and keeps the dry path
latency-matched while bypassed.

The detector compares each candidate with pre/post medians and median absolute
deviation, while rejecting persistent steps and high-variation programme.
Adjacent channel pairs share the detection decision by default so stereo and
surround images remain coherent; interpolation remains channel-specific.

Parameters:

- `enabled`: smoothly crossfade between delayed dry and repaired audio.
- `sensitivity` (1–100): lower values repair more candidates; changes are
  smoothed over 5 ms.
- `link_channels`: link decisions in adjacent channel pairs; disable for fully
  independent channels.

The callback is allocation-free, lock-free, frame-major, and accepts arbitrary
block sizes. Non-finite input is replaced locally with the last finite sample.
See `USAGE.md` and `UI.md` for contracts and controls.
