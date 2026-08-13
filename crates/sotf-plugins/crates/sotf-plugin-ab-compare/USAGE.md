# A/B Compare

## Overview

Fair comparison between two audio processing chains with automatic loudness matching and latency compensation. Each path (A or B) can be a single plugin, a rack (linear chain), or a full graph (DAG topology). Eliminates the "louder sounds better" bias by normalizing loudness between paths using EBU R128 measurement.

## Features

### Mix Control

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Mix | -1.0 to +1.0 | 0.0 | — | Balance between paths: -1.0 = pure A, 0.0 = 50/50, +1.0 = pure B |
| Mix Mode | Potentiometer / Binary | Potentiometer | — | Continuous crossfade or instant A/B switch |
| Selected Path | A / B | A | — | Active path when in Binary mode |
| Bypass | On/Off | Off | — | Bypass both paths, output latency-compensated dry input |
| Transition Time | 1 to 500 | 50 | ms | Crossfade duration for mix changes |

### Path Configuration

Each path (A and B) supports three topology modes:

| Mode | Description |
|------|-------------|
| None | Pass-through (no processing) |
| Plugin | Single plugin with parameters |
| Rack | Linear chain of plugins |
| Graph | Full DAG with nodes and edges |

Available plugin types for paths: EQ, Gain, Compressor, Limiter, Gate, Expander, Denoiser, Loudness Compensation, and others.

The structural band mask can isolate the comparison to a passband. `Band Mask Low` ranges from
20–20,000 Hz (default 20 Hz) and `Band Mask High` uses the same range (default 20,000 Hz); the low
edge must remain below the high edge and active edges must be below Nyquist.

### Auto Gain (Loudness Matching)

Automatically adjusts path B's level to match path A's loudness, ensuring fair comparison.

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Auto Gain | On/Off | On | — | Enable automatic loudness matching |
| Loudness Type | Momentary / Short-term | Momentary | — | EBU R128 measurement window |
| Max Auto Gain | 0 to 24 | 12 | dB | Maximum gain correction applied |
| Gain Smoothing | 1 to 500 | 100 | ms | Smoothing time for gain changes |

### Latency Compensation

Automatically compensates for latency differences between paths A and B using delay lines, ensuring time-aligned comparison.

### Monitoring

Real-time data exposed for display:
- Loudness A / B (LUFS)
- Peak A / B
- Current auto-gain correction (dB)
- Current mix position
- Bypass state

## Demos

### Demo: Comparing EQ Settings

**Scenario:** Evaluating whether an EQ correction improves the sound.
**Before:** Uncertainty about whether EQ changes are positive or just louder.
**After:** Level-matched instant switching between original and EQ-corrected signal.
**Config:**
```json
{
  "path_a": { "type": "None" },
  "path_b": { "type": "Plugin", "plugin_type": "EQ", "parameters": {
    "filters": [{"filter_type": "peak", "frequency": 3000.0, "q": 1.5, "gain_db": -3.0}]
  }},
  "mix_mode": "Binary",
  "auto_gain_enabled": true
}
```

### Demo: Compressor Before vs After

**Scenario:** Judging whether compression improves dynamics without loudness bias.
**Before:** Compressed audio sounds better simply because it's louder.
**After:** Auto-gain matches loudness — you hear only the dynamic difference.
**Config:**
```json
{
  "path_a": { "type": "None" },
  "path_b": { "type": "Plugin", "plugin_type": "Compressor", "parameters": {
    "threshold_db": -18.0, "ratio": 4.0, "attack_ms": 10.0, "release_ms": 100.0
  }},
  "mix_mode": "Binary",
  "auto_gain_enabled": true
}
```

### Demo: Crossfading Between Two Processing Chains

**Scenario:** Smoothly blending between two different processing approaches.
**Before:** No way to gradually transition between processing chains.
**After:** Potentiometer mode allows continuous blend from chain A to chain B.
**Config:**
```json
{
  "path_a": { "type": "Plugin", "plugin_type": "Compressor", "parameters": {
    "threshold_db": -12.0, "ratio": 2.0
  }},
  "path_b": { "type": "Plugin", "plugin_type": "Limiter", "parameters": {
    "threshold_db": -6.0
  }},
  "mix_mode": "Potentiometer",
  "mix": 0.0,
  "auto_gain_enabled": true
}
```

## Presets

### Clean A/B Switch
**Use case:** Quick toggling between processed and unprocessed signal
```json
{
  "path_a": { "type": "None" },
  "path_b": { "type": "None" },
  "mix_mode": "Binary",
  "auto_gain_enabled": true,
  "mix_transition_ms": 50.0
}
```
**Tips:** Set path B to your processing chain. Binary mode with short transition gives clean switching.

### Smooth Crossfade
**Use case:** Gradual blending between two processing approaches
```json
{
  "path_a": { "type": "None" },
  "path_b": { "type": "None" },
  "mix_mode": "Potentiometer",
  "auto_gain_enabled": true,
  "mix_transition_ms": 200.0
}
```
**Tips:** Use potentiometer mode with longer transition for smooth crossfades. Mix at 0.0 gives equal blend.

## Tips & Best Practices

- Always enable auto-gain for fair comparisons — louder signals are perceived as better regardless of actual quality.
- Binary mode is best for critical A/B listening tests; Potentiometer mode is better for exploring the effect of processing.
- The auto-gain system uses EBU R128 loudness (not peak level) for perceptually accurate matching.
- Latency compensation ensures paths with different processing delays are time-aligned.
- Path configs and band-mask cutoffs are structural. The outer host rebuilds the complete A/B node
  off the audio thread, recompiles latency compensation, and swaps it at a safe graph boundary.
- In bypass mode, both processing paths are skipped; the dry signal remains delayed by the plugin's reported latency so parallel host branches stay aligned.
- The default same-source linear crossfade preserves unity for identical A/B paths and avoids the
  +3.01 dB centre boost of an equal-power law. Inverted paths intentionally cancel at centre.
- Diagnostics publish on an elapsed-frame schedule (20 Hz), independent of callback partitioning.

## Signal Flow

```
Input → Fork ──→ Path A (DawHost) → Delay compensation A → ─┐
            └──→ Path B (DawHost) → Delay compensation B → ─┤
                                                             │
Auto Gain measures A & B loudness, adjusts B ───────────────┘
                                                             │
Mix (crossfade or switch) ──→ Output
```
