---
title: "Recording Measurements"
description: "Use SotF's Recording screen to capture room impulse responses and frequency measurements."
---

The Recording screen lets you play a test signal (sine sweep) through your speakers and
record the result with a measurement microphone. The captured impulse response can then
be used in the Convolution plugin or fed into the Room EQ optimizer.

## What You Need

- SotF (Terminal or Desktop)
- A calibrated measurement microphone (e.g., miniDSP UMIK-1, Dayton Audio UMM-6)
- The microphone's calibration file (.txt or .csv)
- Your speakers connected and at their final positions

## Workflow Overview

1. Navigate to the **Recording** screen
2. Select your output device (speakers) and input device (microphone)
3. Configure the sweep signal
4. Record and save the impulse response
5. Use the file in Convolution or Room EQ

## Step-by-Step

### 1. Select Devices

In the Recording screen, choose:
- **Output device** — your speaker output (the device you want to measure)
- **Input device** — your measurement microphone input

If using USB microphones (UMIK-1, etc.), they appear as separate audio devices.

### 2. Configure the Sweep

| Setting | Recommended | Notes |
|---------|-------------|-------|
| Sweep duration | 10–30 s | Longer = better SNR in reverberant rooms |
| Sweep level | -10 to -6 dBFS | Leave headroom; avoid clipping |
| Sweep start freq | 20 Hz | Covers full audio band |
| Sweep end freq | 20 kHz | |

### 3. Set Up the Microphone

- Place the microphone at the primary listening position, ear height
- Point it toward the ceiling (omnidirectional capsule) or at the speaker (cardioid)
- Load the microphone calibration file if you have one (corrects the mic's own response)

### 4. Record

Press **Record** (or the keybinding shown on screen). SotF will:
1. Play the configured sine sweep through the speakers
2. Record the microphone signal simultaneously
3. Deconvolve the recording to extract the impulse response (IR)

The recording takes slightly longer than the sweep duration due to tail capture.

### 5. Save the Result

After recording, SotF displays a summary (peak level, SNR estimate). Save the IR to
a `.wav` file. Use descriptive filenames:

```
living-room-L-seat1.wav
living-room-R-seat1.wav
```

## Using the IR

### In the Convolution Plugin

Load the saved `.wav` file in the Convolution plugin. This applies the room's transfer
function as a convolution filter, useful for:
- Cabinet simulation
- Room reverb
- Custom impulse responses from other tools

### In Room EQ

The Room EQ wizard can import your recorded IR files as measurements. This is the
recommended path for room correction — it accounts for your actual in-room acoustics
rather than relying solely on spinorama anechoic data.

## Tips for Good Measurements

- **Silence the room** — turn off HVAC, fans, and background audio during recording
- **Measure multiple seats** — SotF can average several positions for a more robust correction
- **Record each channel separately** — mute all channels except the one you are measuring
- **Check for clipping** — peak level should stay below 0 dBFS; lower the sweep level if needed
- **Repeat suspicious measurements** — if SNR looks low (< 20 dB), try again
