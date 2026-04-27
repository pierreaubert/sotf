---
title: Room EQ Screen
description: Correct room acoustics with automated per-channel EQ optimization.
---

The Room EQ screen walks you through measuring and correcting your room's acoustic
frequency response. It generates per-channel parametric EQ filters to flatten
in-room response from stereo up to 9.1.6 Atmos configurations.

## Prerequisites

- Speaker frequency response data (from spinorama.org database or local CSV)
- Optional: a calibrated microphone and room impulse response recordings
  (see [Recording Measurements](/guides/recording/))

## Workflow

### Stage 1: Speaker Data

Choose how to provide speaker measurements:

- **Spinorama database** — search by manufacturer and model; SotF fetches anechoic data
  directly from the spinorama.org API
- **Local file** — import a CSV measurement file

Assign measurements to each channel in your setup (L, R, C, Ls, Rs, etc.).

### Stage 2: Room Configuration

Define your speaker layout:

| Setting | Description |
|---------|-------------|
| **Channel configuration** | Stereo, 5.1, 7.1, 9.1.6, etc. |
| **Speaker distance** | Distance from each speaker to the listening position (meters) |
| **Speaker angle** | Horizontal angle of each speaker relative to the listening axis |
| **Subwoofer crossover** | Crossover frequency if LFE channel is used |

### Stage 3: Optimization Parameters

| Setting | Description |
|---------|-------------|
| **Filters per channel** | Number of PEQ bands (5–10 recommended) |
| **Target curve** | Flat, or a tilted house curve |
| **Max boost/cut** | Maximum correction per band (±6 dB is conservative) |
| **Frequency range** | Typically 20 Hz – 16 kHz; narrow for sub-only correction |
| **Algorithm** | COBYLA (fast) or DE (thorough) |

### Stage 4: Optimize

Press **Run**. The optimizer calculates PEQ filters for each channel simultaneously.
Progress is shown per channel. For multi-channel setups this takes a few seconds to
a minute depending on the algorithm and number of filters.

### Stage 5: Review Results

The results screen shows a per-channel before/after frequency response overlay.
Each channel has its own tab. Review each one and optionally adjust individual
filter bands manually.

Key metrics shown per channel:
- **RMS error** — average deviation from target (lower is better; < 1 dB is excellent)
- **Peak error** — worst-case deviation
- **Filter list** — frequency, Q, and gain for each band

### Stage 6: Apply or Export

| Option | Description |
|--------|-------------|
| **Apply** | Load the filters directly into the plugin chain (one EQ plugin per channel) |
| **Export → APO** | EqualizerAPO config files, one per channel |
| **Export → JSON** | SotF preset file for reuse |
| **Export → RME** | RME TotalMix FX channel EQ format |

## Tips

- **Measure your room** — spinorama anechoic data gives a good starting point, but
  actual in-room measurements (via the Recording screen) produce better results
- **Limit bass correction** — room modes below 200 Hz are position-dependent; large
  boosts below 100 Hz can cause driver overload. Prefer cuts over boosts in the bass.
- **Check phase alignment** — the optimizer accounts for inter-channel timing; verify
  the channel delay settings match your actual speaker distances

## See Also

- [Room Correction Guide](/guides/room-correction/)
- [Recording Measurements](/guides/recording/)
