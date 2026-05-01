---
title: "Plugin Presets"
description: "Save, load, and share plugin chain configurations as preset files."
---

A preset is a snapshot of your entire plugin chain — all plugins, their order, and
every parameter value. Presets let you switch between different audio setups instantly
and share configurations with others.

## Saving a Preset

### Terminal

1. Navigate to the **Plugins** screen
2. Press `s` (or the keybinding shown in the help overlay)
3. Enter a filename and confirm

### Desktop

1. Open the **Plugin Rack** or **Plugin Graph** screen
2. Use the **Export** menu → **Save Preset**
3. Choose a location and filename

Presets are saved as `.json` files. You can store them anywhere — SotF doesn't require
a specific directory.

## Loading a Preset

### Terminal

1. Navigate to the **Plugins** screen
2. Press `l` to open the load dialog
3. Browse to your preset file and confirm

### Desktop

1. Open the **Plugin Rack**
2. Use the **Import** menu → **Load Preset**

Loading a preset replaces the entire current plugin chain.

## EqualizerAPO Files

SotF can import EqualizerAPO configuration files (`.txt`). This lets you reuse EQ
settings exported from REW, AutoEQ databases, or other tools:

```
Preamp: -3 dB
Filter 1: ON PK Fc 80 Hz Gain -4.5 dB Q 1.41
Filter 2: ON PK Fc 180 Hz Gain 3.2 dB Q 2.0
Filter 3: ON HS Fc 6000 Hz Gain 1.0 dB Q 0.707
```

Import the same way as a regular preset — SotF auto-detects the format.

## Preset File Format

Presets are JSON arrays of plugin configurations:

```json
[
  {
    "plugin_type": "EQ",
    "parameters": {
      "filters": [
        { "filter_type": "peak", "frequency": 80.0, "q": 1.41, "gain_db": -4.5 },
        { "filter_type": "peak", "frequency": 2500.0, "q": 2.0, "gain_db": 2.0 }
      ]
    }
  },
  {
    "plugin_type": "Gain",
    "parameters": { "gain_db": -1.5 }
  }
]
```

You can edit these files by hand or generate them programmatically.

## Sharing Presets

Preset files are self-contained JSON — share them by copying the file.
Common sharing locations:
- [GitHub Discussions](https://github.com/pierreaubert/sotf/discussions) — community preset sharing
- EqualizerAPO format is interoperable with many other tools

## Tips

- **Name presets descriptively**: `hifiman-he400se-harman.json` beats `preset1.json`
- **Version your presets**: include the SotF version or date in the filename if you iterate
- **Keep a "flat" preset**: a single Gain plugin at 0 dB is useful as a quick bypass baseline
