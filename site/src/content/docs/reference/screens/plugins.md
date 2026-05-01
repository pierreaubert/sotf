---
title: Plugins Screen
description: Add, configure, reorder, and remove audio processing plugins.
---

The Plugins screen manages the audio processing chain. Plugins process audio in the
order they appear — top to bottom (Terminal) or left to right in rack view (Desktop).

## Navigation

| Key (Terminal) | Action |
|-----------|--------|
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `a` | Open the plugin browser to add a plugin |
| `d` | Remove the selected plugin |
| `Enter` | Open the selected plugin's parameter editor |
| `e` | Toggle the selected plugin enabled/disabled |
| `J` | Move selected plugin down (reorder) |
| `K` | Move selected plugin up (reorder) |
| `s` | Save the current chain as a preset |
| `l` | Load a preset or APO EQ file |

## Adding a Plugin

Press `a` to open the plugin browser. Use `/` to search by name. Press `Enter` to
add the highlighted plugin to the end of the chain.

## Editing Parameters

Press `Enter` on any plugin to open its parameter editor. Navigate parameters with
`j`/`k`, adjust values with `←`/`→` or type a number directly.

Each parameter shows its current value, range, and unit. Changes take effect in
real time — you can hear the result while you edit.

## Enabling / Disabling

Press `e` to toggle a plugin on or off without removing it from the chain.
Disabled plugins are shown dimmed. This is useful for A/B comparison: toggle the
EQ on and off to hear the difference.

## Channel Count

Some plugins change the channel count of the signal:
- **Upmixer** — 2 channels → 5 channels
- **Downmix** — N channels → 2 channels
- **Binaural** — N channels → 2 channels

If a plugin expects a channel count that doesn't match the incoming signal,
SotF suspends it automatically. The plugin shows a warning indicator.
Reorder your chain so channel counts match at each stage.

## Presets and APO Files

See the [Plugin Presets guide](/guides/plugin-presets/) for saving, loading, and
sharing plugin chain configurations. SotF can also import EqualizerAPO `.txt` files.
