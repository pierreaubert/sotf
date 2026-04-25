# 0.5.2

## Fixes

- Process blocks larger than the FFT size by chunking them through the overlap-add path.
- Avoid silently passing oversized blocks through dry while still reporting FIR latency.
- Add regression coverage that verifies large blocks are processed.
