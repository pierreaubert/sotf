---
title: roomeq
description: Multi-channel room equalization optimizer CLI — full flag reference.
---

Generates per-channel DSP correction chains for speaker systems.
See the [RoomEQ CLI guide](/guides/roomeq-cli/) for task-oriented examples and config file format.

## Synopsis

```
roomeq --config <CONFIG> --output <OUTPUT> [OPTIONS]
roomeq --schema <input|output>
roomeq --convert <DSP_CHAIN> --export-format <FORMAT>
```

## Flags

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `-c` / `--config` | Yes* | — | Room configuration JSON file |
| `-o` / `--output` | Yes* | — | Output DSP chain JSON file |
| `--sample-rate` | No | `48000` | Filter design sample rate (Hz) |
| `--export-format` | No | — | Also export to external format (see below) |
| `--export-path` | No | auto | Export file path (auto from format extension) |
| `--override-config` | No | — | JSON that overrides any section of the main config |
| `--dry-run` | No | — | Validate config and check files without optimizing |
| `--schema` | No | — | Print JSON schema (`input` or `output`) and exit |
| `--convert` | No | — | Convert existing DSP chain JSON to export format |

*Not required when using `--schema` or `--convert`.

## Export Formats (`--export-format`)

| Value | Extension | Target application |
|-------|-----------|-------------------|
| `camilladsp` | `.yaml` | CamillaDSP |
| `apo` | `.txt` | EqualizerAPO, Peace GUI, PipeWire parametric-EQ |
| `easyeffects` | `.json` | EasyEffects |
| `wavelet` | `.txt` | Wavelet (Android GraphicEQ) |
| `pipewire` | `.conf` | PipeWire filter-chain |
| `roon` | `.json` | Roon DSP Engine |

## Environment

`RUST_LOG` controls log verbosity: `error`, `warn`, `info` *(default)*, `debug`.

```bash
RUST_LOG=debug roomeq --config room.json --output out.json
```

## Config Quick Reference

Minimal `config.json`:
```json
{
  "speakers": {
    "left":  "measurements/left.csv",
    "right": "measurements/right.csv"
  },
  "optimizer": {
    "num_filters": 7,
    "algorithm":   "autoeq:de",
    "min_freq":    20.0,
    "max_freq":    1600.0
  }
}
```

See [`INPUT_FORMAT.md`](https://github.com/pierreaubert/sotf/blob/master/crates/autoeq/bin/roomeq/INPUT_FORMAT.md)
for the complete configuration schema, or dump it with:
```bash
roomeq --schema input
```

## Output Format

The output JSON contains a `channels` map and a `metadata` block:

```json
{
  "channels": {
    "left": {
      "plugins": [
        { "plugin_type": "gain", "parameters": { "gain_db": -2.5 } },
        { "plugin_type": "eq",   "parameters": { "filters": [...] } }
      ]
    }
  },
  "metadata": {
    "pre_score": 0.42,
    "post_score": 0.91,
    "algorithm": "autoeq:de",
    "iterations": 50000
  }
}
```

Load the output in SotF by importing the JSON file from the Plugin Rack.
