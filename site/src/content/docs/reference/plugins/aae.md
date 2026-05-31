---
title: "Active Acoustic Enhancement"
description: "Active acoustic enhancement using psychoacoustic processing to improve perceived clarity, presence, and depth."
---

Active acoustic enhancement using psychoacoustic processing to improve perceived clarity, presence, and depth.

## Parameters


### Spatial

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Speaker Config | Choice (5.0, 5.1, 7.1, 5.1.2, 5.1.4, 7.1.2, 7.1.4, 9.1.4, 9.1.6) | 9 options | 5.1 | - | Output speaker layout |
| Envelopment | Float | 0 .. 1 | 0.7 | x | Rear/surround vs front reverb balance |
| Height Amount | Float | 0 .. 1 | 0.5 | x | Height channel contribution |

### Room

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Room Size | Float | 0.2 .. 3 | 1 | x | Scales all delay line lengths |
| RT60 | Float | 0.3 .. 6 | 1.8 | s | Mid-frequency reverberation time |
| Bass Ratio | Float | 0.8 .. 2 | 1.2 | x | RT60_bass / RT60_mid ratio |
| Treble Ratio | Float | 0.2 .. 1 | 0.5 | x | RT60_treble / RT60_mid ratio |
| Pre-delay | Float | 0 .. 100 | 20 | ms | Gap before first reflection |
| Room Preset | Choice (small, medium, large, cathedral) | 4 options | medium | - | Early reflection tap configuration |

### Levels

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Dry Level | Float | 0 .. 1 | 0.5 | x | Direct dry output level |
| ER Level | Float | 0 .. 1 | 0.3 | x | Early reflection level |
| Late Level | Float | 0 .. 1 | 0.2 | x | Late reverb (FDN) level |
| LFE Level | Float | 0 .. 1 | 0.2 | x | Bass sent to LFE channel |

### Modulation

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Mod Depth | Float | 0 .. 1 | 0.5 | x | FDN time-variant delay modulation (Griesinger) |
| ER Mod Depth | Float | 0 .. 1 | 0.3 | x | Early reflection tap modulation |

### Character

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Input Diffusion | Float | 0 .. 1 | 0.7 | x | Pre-FDN allpass diffusion |

### Intelligence

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Content Aware | Bool | On / Off | On | - | Enable speech detection for reverb ducking |
| Dialogue Atten. | Float | 0 .. 12 | 6 | dB | Reverb reduction during detected speech |
| Safety Limit | Float | 0 .. 12 | 6 | dB | FDN feedback limiter threshold |

### Auto Gain

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Auto Gain | Bool | On / Off | Off | - | Match rendered output loudness to the stereo input |
| AG Max | Float | 0 .. 24 | 12 | dB | Maximum auto gain correction |
| AG Smoothing | Float | 10 .. 500 | 100 | ms | Auto gain transition time |

### Diagnostic

| Parameter | Type | Range | Default | Unit | Description |
|-----------|------|-------|---------|------|-------------|
| Bypass | Bool | On / Off | Off | - | Pass-through mode |
| Solo Early | Bool | On / Off | Off | - | Hear only early reflections |
| Solo Late | Bool | On / Off | Off | - | Hear only late reverb |

:::note
**Structural parameters** (Speaker Config) require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout.
:::
