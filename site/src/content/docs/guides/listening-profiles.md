---
title: "Listening Profiles"
description: "Create and switch between named plugin chain configurations for different listening scenarios."
---

A listening profile is a named preset that groups all your plugin settings for a particular
scenario — headphone listening, desktop speakers, late-night low volume, home theater, etc.
Profiles let you switch your entire audio setup in seconds.

## Common Profile Examples

| Profile | Typical plugin chain |
|---------|----------------------|
| **Headphones** | EQ (Harman target) → Crossfeed → Loudness Compensation |
| **Desktop Speakers** | Room EQ → Gain |
| **Late Night** | Loudness Compensation (boosted) → Limiter |
| **Home Theater** | Room EQ → Upmixer → Gain |
| **Flat / Bypass** | Gain (0 dB) |

## Creating a Profile

1. Set up your plugin chain for the scenario you want to save
2. Open the **Plugins** screen (TUI) or **Plugin Rack** (Desktop)
3. Save the current chain as a preset file — use a descriptive name:
   ```
   headphones-hifiman-he400se.json
   speakers-kef-r3-room-eq.json
   late-night.json
   ```

See the [Plugin Presets guide](/guides/plugin-presets/) for detailed save/load instructions.

## Switching Profiles

### TUI

Press `l` on the Plugins screen to open the load dialog and navigate to any preset file.
The switch takes effect immediately with no audio interruption (SotF cross-fades between chains).

### Desktop (GPUI)

Use **Import → Load Preset** from the Plugin Rack, or set up keyboard shortcuts for
frequently used presets in Settings.

### macOS System-Wide (Daemon)

If you use the HAL driver for system-wide audio, you can switch presets from the menubar
app without restarting anything. The daemon hot-reloads the new plugin chain.

## Organizing Profile Files

SotF doesn't enforce a directory structure, but a consistent layout helps:

```
~/.config/sotf/presets/
  headphones/
    hifiman-he400se-harman.json
    beyerdynamic-dt1990-bright.json
  speakers/
    kef-r3-room-eq.json
    flat.json
  scenarios/
    late-night.json
    home-theater.json
```

## Automating Profile Switches

On macOS, you can trigger profile switches via the daemon's SIGHUP reload:

```bash
# Switch to home theater profile
cp ~/.config/sotf/presets/scenarios/home-theater.json ~/.config/sotf/active.json
kill -HUP $(pgrep sotf-daemon)
```

Wrap this in a shell script or bind it to a keyboard shortcut with Automator/BetterTouchTool.

## Tips

- **Start from a working chain** — build each profile incrementally and verify it sounds right
  before saving
- **Keep a flat profile** — useful for A/B comparison and debugging
- **Document your profiles** — add a comment field in the JSON (SotF ignores unknown fields)
  explaining what each profile is optimized for
