# Mono to Stereo

## Overview

Converts a mono signal into a stereo signal using FFT-based random-phase decorrelation. Unlike simple panning or Haas-effect tricks, this plugin creates a perceptually wide stereo image from mono content by applying frequency-dependent phase randomization, ensuring the left and right channels are decorrelated without altering the tonal balance.

## Features

### Stereo Width Control

Controls how much decorrelation is applied to the right channel. At width 0, both channels are identical (mono). At width 1, the right channel is fully decorrelated from the left, creating maximum stereo spread.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Width | 0.0 to 1.0 | 0.5 | — | Stereo width. 0 = mono, 1 = full decorrelation |

### Haas Delay

Adds a small time offset between left and right channels for additional spatial widening based on the Haas effect (precedence effect).

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Haas Delay | 0.0 to 5.0 | 1.5 | ms | Inter-channel delay for Haas-effect widening |

### Complementary EQ

Applies subtle spectral differences between L and R channels to enhance the perception of width without relying solely on phase decorrelation.

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Comp EQ | On/Off | On | — | Enable complementary EQ for additional width |
| Comp EQ Depth | 0.0 to 3.0 | 1.0 | dB | Amount of spectral complementarity between L/R |

### Decorrelation Band

Defines the frequency range over which random-phase decorrelation is applied. Frequencies outside this range are kept identical in both channels (mono-compatible).

**Parameters:**

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Decor Low | 100 to 500 | 300 | Hz | Lower frequency bound for decorrelation |
| Decor High | 1000 to 5000 | 2000 | Hz | Upper frequency bound for decorrelation |

## Demos

### Demo: Mono Vocal to Stereo

**Scenario:** A mono vocal recording needs to sit in a stereo mix without sounding narrow.
**Before:** Vocal is centered and flat, fighting with other centered elements.
**After:** Vocal has a natural stereo presence, occupying space in the mix without obvious doubling artifacts.
**Config:**
```json
{
  "stereo_width": 0.4,
  "comp_eq_depth_db": 1.0
}
```

### Demo: Full Width for Ambient Textures

**Scenario:** A mono synth pad needs to fill the entire stereo field.
**Before:** Synth pad sounds narrow and small.
**After:** Wide, immersive stereo field that fills the speakers.
**Config:**
```json
{
  "stereo_width": 1.0,
  "comp_eq_depth_db": 1.5
}
```

### Demo: Subtle Widening for Podcast

**Scenario:** A mono podcast recording needs slight stereo widening for a more spacious feel.
**Before:** Flat mono recording.
**After:** Slightly wider feel without obvious stereo effects — sounds natural on headphones.
**Config:**
```json
{
  "stereo_width": 0.2,
  "comp_eq_depth_db": 0.5
}
```

## Presets

### Subtle Width
**Use case:** Gentle widening for mono vocals or dialogue
```json
{
  "stereo_width": 0.3,
  "comp_eq_depth_db": 0.8
}
```
**Tips:** Check mono compatibility — fold to mono and ensure no cancellation artifacts.

### Natural Stereo
**Use case:** General-purpose mono-to-stereo conversion
```json
{
  "stereo_width": 0.5,
  "comp_eq_depth_db": 1.0
}
```
**Tips:** Good starting point for most material. Adjust width to taste.

### Wide Spread
**Use case:** Full stereo field for pads, synths, or ambient content
```json
{
  "stereo_width": 0.8,
  "comp_eq_depth_db": 1.5
}
```
**Tips:** At high width values, check that the sound still collapses to mono gracefully.

### Maximum Width
**Use case:** Effect-level stereo spread
```json
{
  "stereo_width": 1.0,
  "comp_eq_depth_db": 2.0
}
```
**Tips:** Full decorrelation — the left and right channels will be completely different. Not mono-compatible.

## Tips & Best Practices

- The plugin introduces latency (2048 samples / ~46 ms at 44.1 kHz) due to FFT processing.
- Width = 0 produces perfect mono (L = R). Use this to verify the plugin is transparent.
- Check mono compatibility when using high width values — fold to mono and listen for cancellation.
- The decorrelation band (300-2000 Hz by default) keeps bass and highs mono-compatible.
- Lower the decorrelation low frequency to include more bass in the stereo field (can reduce mono compatibility).
- For headphone listening, lower width values (0.2-0.4) often sound more natural than full width.
- The complementary EQ adds subtle tonal differences between channels for a more convincing stereo image.

## Signal Flow

```
Mono Input → FFT Analysis → Windowed STFT
                                │
                   ┌────────────┴────────────┐
                   │                         │
              Left (direct)         Right (decorrelated)
                   │                         │
              IFFT + OLA              IFFT + OLA
                   │                         │
                   └────────┬────────────────┘
                            │
                     Width Crossfade
                   (R = L×(1-w) + decor×w)
                            │
                     Stereo Output [L, R]
```
