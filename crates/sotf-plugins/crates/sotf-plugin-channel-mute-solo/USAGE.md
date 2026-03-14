# Channel Mute/Solo

## Overview

A simple per-channel mute, solo, and dim plugin with smooth fading to prevent clicks. Supports any number of channels. Used for isolating individual channels during monitoring, debugging surround setups, or creating custom channel mixes.

## Features

### Channel States

Each channel has independent state flags:

| State | Target Gain | Description |
|-------|-------------|-------------|
| Normal | 1.0 | Full volume |
| Muted | 0.0 | Silenced (smooth fade-out) |
| Dimmed | 0.1 (-20 dB) | Reduced level for background monitoring |
| Soloed | 1.0 (others → 0.0) | Isolates this channel, mutes all others |

### Control

| Parameter | Range | Default | Unit | Description |
|-----------|-------|---------|------|-------------|
| Enabled | On/Off | On | — | Master enable/disable for the plugin |

### Solo Priority

When any channel is soloed, all non-soloed channels are silenced regardless of their mute/dim state. Multiple channels can be soloed simultaneously.

### Smooth Transitions

All state changes use a 5 ms fade to prevent audible clicks. The smoother interpolates linearly from current gain to target gain.

## Demos

### Demo: Isolating the Center Channel

**Scenario:** Checking dialogue clarity in a 5.1 mix by soloing the center channel.
**Before:** Full 5.1 mix with all channels active.
**After:** Only the center channel plays — dialogue is isolated for evaluation.

### Demo: Muting the Subwoofer

**Scenario:** Temporarily muting the LFE channel to check bass management.
**Before:** Full-range playback with subwoofer active.
**After:** LFE muted — reveals how much bass content is in the main channels.

### Demo: Dimming Surrounds

**Scenario:** Reducing surround channels to focus on front soundstage.
**Before:** Full surround immersion may mask front imaging issues.
**After:** Surrounds at -20 dB — front channels dominate, surround still provides context.

## Tips & Best Practices

- The plugin uses SIMD-optimized gain application for minimal CPU impact.
- When disabled and all smoothers have settled to 1.0, processing is bypassed entirely.
- Channel states are serialized as JSON for preset storage.
- Smooth fading prevents any audible artifacts when toggling states.
- Place this plugin after the upmixer/matrix for surround channel monitoring.
- Solo is exclusive: when active, it overrides mute and dim on other channels.

## Signal Flow

```
For each channel:
  target = solo_active_anywhere ?
             (is_soloed ? 1.0 : 0.0) :
             (is_muted ? 0.0 : is_dimmed ? 0.1 : 1.0)

  gain = smoother.advance(target)  // 5ms fade
  output[ch] = input[ch] × gain
```
