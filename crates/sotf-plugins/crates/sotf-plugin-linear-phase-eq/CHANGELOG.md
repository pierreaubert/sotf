# Unreleased

## Fixes

- Fix DC magnitude hardcoded to 0 dB in FIR design; now computed from actual biquad DC gain.
- Fix lowpass and highpass bands with 0 dB gain being silently skipped in FIR design.

# 0.5.2

## Fixes

- Process blocks larger than the FFT size by chunking them through the overlap-add path.
- Avoid silently passing oversized blocks through dry while still reporting FIR latency.
- Add regression coverage that verifies large blocks are processed.
