# Mono to Stereo

## Overview

Converts a mono signal into stereo with a causal all-pass cascade. The right channel receives a
frequency-shaped phase response whose magnitude is unity in steady state; the left channel remains
direct. This avoids FFT circular wrap and random-bin pre-echo while retaining optional Haas widening.

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

### Decorrelation Band

Defines the frequency range around which the causal all-pass phase rotation is concentrated.
These topology controls, including `freq_dependent`, are applied by rebuilding the plugin graph.

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
  "freq_dependent": true
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
  "freq_dependent": true
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
  "freq_dependent": true
}
```

## Presets

### Subtle Width
**Use case:** Gentle widening for mono vocals or dialogue
```json
{
  "stereo_width": 0.3,
  "freq_dependent": true
}
```
**Tips:** Check mono compatibility — fold to mono and ensure no cancellation artifacts.

### Natural Stereo
**Use case:** General-purpose mono-to-stereo conversion
```json
{
  "stereo_width": 0.5,
  "freq_dependent": true
}
```
**Tips:** Good starting point for most material. Adjust width to taste.

### Wide Spread
**Use case:** Full stereo field for pads, synths, or ambient content
```json
{
  "stereo_width": 0.8,
  "freq_dependent": true
}
```
**Tips:** At high width values, check that the sound still collapses to mono gracefully.

### Maximum Width
**Use case:** Effect-level stereo spread
```json
{
  "stereo_width": 1.0,
  "freq_dependent": false
}
```
**Tips:** Full decorrelation — the left and right channels will be completely different. Not mono-compatible.

## Tips & Best Practices

- The causal all-pass path reports zero host-compensated latency. Haas delay remains an intentional right-channel effect.
- Width = 0 produces perfect mono (L = R). Use this to verify the plugin is transparent.
- Check mono compatibility when using high width values — fold to mono and listen for cancellation.
- The decorrelation band (300-2000 Hz by default) keeps low bass highly correlated.
- Lower the decorrelation low frequency to include more bass in the stereo field (can reduce mono compatibility).
- For headphone listening, lower width values (0.2-0.4) often sound more natural than full width.

## Signal Flow

```
Mono Input ───────────────→ Left (direct)
     │
     └→ Causal all-pass cascade → optional Haas delay → Right

Width changes the stable all-pass coefficients; width 0 uses an exact L=R duplicate fast path.
```
