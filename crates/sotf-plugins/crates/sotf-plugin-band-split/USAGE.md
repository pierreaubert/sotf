# Band Split

## Overview

Splits an input signal into two frequency bands (low and high) using phase-coherent Linkwitz-Riley crossover filters. The output has 2× the input channels — low bands first, then high bands. Used for parallel processing of different frequency ranges (e.g., compressing bass separately from treble).

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
- Linkwitz-Riley crossovers are phase-coherent — summing the bands perfectly reconstructs the original signal.
- Frequency changes are smoothed with a LogSmoother (20 ms) for click-free transitions.
- Filters are rebuilt when frequency drifts more than 0.1 Hz from the target.
- LR48 uses 2× the CPU of LR24 due to double the filter sections.
- The output channel count doubles — downstream plugins must handle the increased channel count.

## Signal Flow

```
Input (N channels) → [Lowpass LR chain] → Output channels 0..N-1 (low band)
                   → [Highpass LR chain] → Output channels N..2N-1 (high band)
```
