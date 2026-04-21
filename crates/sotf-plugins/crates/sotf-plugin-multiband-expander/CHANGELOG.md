# Changelog

## 0.5.22

- Added lookahead support (`lookahead_ms` parameter, 0–20 ms). Detection runs
  on the current sample while gain is applied to a delayed copy, letting the
  envelope "see" transients before they reach the output. Per-band, per-channel
  `LookaheadBuffer` circular delays, with latency correctly reported to the host.
- Added `lookahead_ms` to `GLOBAL_PARAMS` (index 17) and the multiband LAYOUT
  TIMING group.

## 0.5.21

- Initial multiband expander with LR4 crossovers, per-band dynamics,
  time-domain and spectral processing modes.
