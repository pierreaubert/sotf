# Band Split

## Overview

Splits an input signal into frequency bands using Linkwitz-Riley crossover filters. The output is band-major: every channel of band 0, then every channel of band 1, and so on. Used for parallel processing of different frequency ranges (e.g., compressing bass separately from treble).

## Features

### Crossover

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Frequency | 20 to 20000 | 300 | Hz | Crossover frequency between low and high bands |
| Type | LR24 / LR48 | LR24 | — | Crossover slope: LR24 (24 dB/oct) or LR48 (48 dB/oct) |

### Linkwitz-Riley Filters

| Type | Slope | Sections | Description |
|------|-------|----------|-------------|
| LR24 | 24 dB/octave | 2 cascaded biquads | Standard crossover, lower CPU |
| LR48 | 48 dB/octave | 4 cascaded biquads | Steeper crossover, better band isolation |

### Channel Layout

For N input channels, the output has 2N channels:
- Channels 0..N-1: Low-pass filtered signal
- Channels N..2N-1: High-pass filtered signal

Example: 2-channel stereo input → 4-channel output [low-L, low-R, high-L, high-R]

## Demos

### Demo: Parallel Compression

**Scenario:** Compress bass and treble independently for better dynamics control.
**Before:** Single compressor affects the entire spectrum — bass pumping affects treble.
**After:** Band Split → Compress low band only → Band Merge. Bass compression doesn't affect treble clarity.
**Config:**
```json
{
  "frequency": 300.0,
  "type": "LR24"
}
```

### Demo: Steep Frequency Split

**Scenario:** Isolating sub-bass from everything else for separate processing.
**Before:** Sub-bass and mid-bass are interleaved in the same signal.
**After:** Clean sub-bass isolation with minimal bleed using LR48 crossover.
**Config:**
```json
{
  "frequency": 80.0,
  "type": "LR48"
}
```

## Presets

### Standard Split
**Use case:** General-purpose frequency splitting
```json
{
  "frequency": 300.0,
  "type": "LR24"
}
```
**Tips:** 300 Hz separates bass from midrange. Good for parallel compression setups.

### Sub-Bass Isolation
**Use case:** Isolating sub-bass frequencies
```json
{
  "frequency": 80.0,
  "type": "LR48"
}
```
**Tips:** LR48 slope minimizes bleed between bands.

## Tips & Best Practices

- Always pair Band Split with Band Merge to recombine the bands after processing.
- A two-band Linkwitz-Riley split has complementary low/high responses. Cascaded three- and
  four-band splits have unequal group delays and do not perfectly null against the original signal.
- Frequency changes follow a 20 ms logarithmic smoother. Filter coefficients
  are updated at a persistent 6 kHz control rate, avoiding audio-rate
  trigonometric redesign while remaining independent of callback partitions.
- Frequency automation is smoothed, while LR24/LR48 type changes require rebuilding the plugin.
- LR48 uses 2× the CPU of LR24 due to double the filter sections.
- Frequencies must be finite, strictly ascending, and no higher than the lower
  of 20 kHz or 0.49 × sample rate. Invalid automation is rejected transactionally.
- The output channel count doubles — downstream plugins must handle the increased channel count.

## Host and realtime contract

- Initialize before processing. Callback sample rate must match initialization,
  and input/output buffers must have the exact overflow-checked interleaved size.
- Invalid static or dynamic frequency updates fail transactionally. LR24/LR48
  changes require graph replacement; an exact write of the current type is a no-op.
- Steady processing and live frequency/band-gain setters allocate no memory.
  Reset snaps parameter ramps and filter coefficients to their target state.
- Two-band sums are magnitude-complementary but phase-shifted. Cascaded
  multiband sums have unequal group delay and are not phase-perfect.

## Signal Flow

```
Input (N channels) → [Lowpass LR chain] → Output channels 0..N-1 (low band)
                   → [Highpass LR chain] → Output channels N..2N-1 (high band)
```
