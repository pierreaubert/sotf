# Convolution

## Overview

A partitioned FFT-based convolution plugin for applying impulse responses (IRs) to audio in real time. Loads WAV impulse response files and convolves the input signal with the IR using overlap-add processing. Used for speaker simulation, reverb, and room correction. Supports multi-channel IRs with per-channel processing.

## Features

### IR Loading

Load a WAV, FLAC, or AIFF impulse response. It is resampled when needed and prepared off-thread.
Files must carry valid sample-rate metadata and fit the 32-channel, 30-second, 512 MiB backend
budget.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| IR File | File path | — | — | Path to the impulse response WAV file |

### Output

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Mix | 0 to 1.0 | 1.0 | — | Dry/wet blend (0 = dry, 1 = fully convolved) |
| Gain | -20 to 20 | 0 | dB | Output gain adjustment |

### Multi-Channel Support

When the IR file has multiple channels, each audio channel is convolved with the corresponding IR channel. If the IR is mono, it is applied identically to all channels.

## Demos

### Demo: Speaker Impulse Response

**Scenario:** Simulating the response of a specific speaker using its measured impulse response.
**Before:** Flat audio signal with no speaker character.
**After:** Audio sounds as if played through the modeled speaker with its natural frequency response and phase characteristics.
**Config:**
```json
{
  "ir_file": "/path/to/speaker_ir.wav",
  "mix": 1.0,
  "gain_db": 0.0
}
```

### Demo: Room Correction

**Scenario:** Applying an inverse room correction filter measured with REW or similar tool.
**Before:** Room modes cause boomy bass and harsh reflections.
**After:** Corrected response with smoother frequency response and reduced room colorations.
**Config:**
```json
{
  "ir_file": "/path/to/room_correction.wav",
  "mix": 1.0,
  "gain_db": -3.0
}
```

### Demo: Reverb Effect

**Scenario:** Adding a concert hall reverb to dry recordings.
**Before:** Dry, studio-recorded audio with no spatial character.
**After:** Natural reverb tail from the concert hall IR adds depth and space.
**Config:**
```json
{
  "ir_file": "/path/to/concert_hall.wav",
  "mix": 0.3,
  "gain_db": 0.0
}
```

## Presets

### Full Convolution
**Use case:** 100% wet signal for speaker simulation or room correction
```json
{
  "mix": 1.0,
  "gain_db": 0.0
}
```
**Tips:** Standard setting for correction filters where you want the full processed signal.

### Subtle Reverb
**Use case:** Adding light reverb without losing dry signal presence
```json
{
  "mix": 0.2,
  "gain_db": 0.0
}
```
**Tips:** Low mix values blend in just enough reverb for spatial enhancement.

### Heavy Reverb
**Use case:** Immersive reverb effect
```json
{
  "mix": 0.5,
  "gain_db": -3.0
}
```
**Tips:** Negative gain compensates for the energy added by the reverb tail.

## Tips & Best Practices

- UPC and normal NUPC add 1024 samples of latency (~21 ms at 48 kHz). Dry audio remains delayed by
  the same amount while empty, loading, failed, cleared, or at mix=0. A configured time-domain NUPC
  head reports zero latency.
- Longer IRs require more partitions and more CPU — keep IRs under a few seconds for real-time use.
- The IR is pre-transformed to frequency domain at load time — switching IRs is not instantaneous.
- Use Mix at 1.0 for correction filters and lower values (0.1–0.5) for reverb effects.
- The Gain parameter adjusts the output level — use it to compensate for IRs that are louder or quieter than unity.
- WAV, FLAC, and AIFF PCM IRs are supported.
- Failed replacements preserve the last working IR. Hosts can inspect `load_status()` for
  idle/loading/ready/failed state.
- SIMD-optimized complex multiply-accumulate is used for the frequency-domain convolution.
- Mono IRs are automatically applied to all channels. Multi-channel IRs require matching channel counts.

## Signal Flow

```
IR File → Partition (1024-sample blocks) → FFT (pre-computed at load time)
                                                    ↓
Input → Partition → FFT → Frequency Domain Line (ring buffer)
                                    ↓
                    For each partition: FDL[p] × IR[p]  (SIMD complex multiply)
                                    ↓
                              Sum all partitions
                                    ↓
                              IFFT → Overlap-Add
                                    ↓
                    Output = Dry × (1-mix) + Wet × mix × gain
```
