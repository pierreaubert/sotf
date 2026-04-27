---
title: "RoomEQ CLI"
description: "Use the roomeq command-line tool to generate multi-channel DSP correction chains for speakers and subwoofers."
---

The `roomeq` binary takes a JSON configuration file describing your speaker measurements and
room setup, runs the optimizer, and writes a DSP chain JSON that can be loaded directly into
SotF's audio engine or exported to third-party tools.

## Quick Start

### Stereo System

```bash
roomeq --config stereo.json --output stereo-dsp.json
```

`stereo.json`:
```json
{
  "speakers": {
    "left": "measurements/left.csv",
    "right": "measurements/right.csv"
  },
  "optimizer": {
    "num_filters": 7,
    "algorithm": "autoeq:de",
    "min_freq": 20.0,
    "max_freq": 1600.0
  }
}
```

### 2.1 with Subwoofer + Harman Target

```bash
roomeq --config 2.1.json --output 2.1-dsp.json
```

`2.1.json`:
```json
{
  "system": {
    "model": "stereo",
    "speakers": { "L": "left", "R": "right", "LFE": "sub" },
    "subwoofers": {
      "config": "single",
      "crossover": "bass_xo",
      "sub": "L"
    }
  },
  "speakers": {
    "left": "measurements/left.csv",
    "right": "measurements/right.csv",
    "sub": "measurements/sub.csv"
  },
  "crossovers": {
    "bass_xo": { "type": "LR24", "frequency": 80.0 }
  },
  "optimizer": {
    "num_filters": 7,
    "algorithm": "autoeq:de",
    "target_response": { "shape": "harman" },
    "phase_alignment": { "enabled": true }
  }
}
```

## CLI Flags

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `-c` / `--config` | Yes* | - | Input room configuration JSON |
| `-o` / `--output` | Yes* | - | Output DSP chain JSON |
| `--sample-rate` | No | `48000` | Filter design sample rate (Hz) |
| `--export-format` | No | - | Also export to an external format (see below) |
| `--export-path` | No | auto | Export output path (auto-derived from format) |
| `--override-config` | No | - | JSON file that overrides any section of the config |
| `--dry-run` | No | - | Validate config and check files without optimizing |
| `--schema input\|output` | No | - | Print JSON schema and exit |
| `--convert` | No | - | Convert existing DSP chain JSON to an export format (no optimization) |

*Not required when using `--schema` or `--convert`.

## Export Formats

Use `--export-format` to produce output for external applications alongside the SotF JSON:

| Value | Output | Use with |
|-------|--------|----------|
| `camilladsp` | YAML config | CamillaDSP |
| `apo` | `.txt` | EqualizerAPO, Peace GUI, PipeWire parametric-EQ module |
| `easyeffects` | JSON preset | EasyEffects |
| `wavelet` | `.txt` | Wavelet (Android GraphicEQ) |
| `pipewire` | `.conf` | PipeWire filter-chain |
| `roon` | JSON | Roon DSP Engine |

```bash
roomeq --config room.json --output room-dsp.json --export-format apo --export-path room-eq.txt
```

Convert an existing output without re-optimizing:
```bash
roomeq --convert room-dsp.json --export-format camilladsp --export-path room.yaml
```

## Measurement CSV Format

```csv
freq,spl,phase
20,75.0,45.2
50,78.0,30.1
100,80.0,15.5
20000,60.0,-90.3
```

Phase data is optional but required for accurate phase alignment and multi-driver crossover
optimization.

## Speaker Types

| Type | Config key | Use case |
|------|-----------|----------|
| Single speaker | path string or `{ "path": "..." }` | Standard speaker per channel |
| Group (multi-driver) | `{ "measurements": [...], "crossover": "..." }` | Active 2-way/3-way speaker |
| MultiSub | `{ "subwoofers": [...] }` | Multiple subwoofers with gain/delay optimization |
| DBA | `{ "front": [...], "rear": [...] }` | Double Bass Array |
| Cardioid | `{ "front": "...", "rear": "...", "separation_meters": 0.5 }` | Gradient cardioid sub pair |

## Optimizer Parameters

The `optimizer` block in the config drives the optimization. Key fields:

| Field | Default | Description |
|-------|---------|-------------|
| `num_filters` | `7` | PEQ filters per channel |
| `algorithm` | `"autoeq:de"` | Optimization algorithm (`autoeq:de`, `cobyla`, `nlopt:isres`, …) |
| `loss_type` | `"flat"` | `"flat"`, `"score"`, or `"epa"` |
| `min_q` / `max_q` | `0.5` / `6.0` | Q factor range |
| `min_db` / `max_db` | `-12` / `4` | Gain range (dB) |
| `min_freq` / `max_freq` | `20` / `1600` | Frequency range (Hz) |
| `max_iter` | `50000` | Maximum optimizer iterations |
| `refine` | `true` | Two-stage optimization (DE global + COBYLA local) |
| `psychoacoustic` | `true` | ERB-weighted loss for perceptual relevance |
| `asymmetric_loss` | `true` | Penalize peaks 2× more than dips |

## Target Response

Control the in-room tonal target under `optimizer.target_response`:

| Shape | Description |
|-------|-------------|
| `"flat"` | 0 dB flat target |
| `"harman"` | Harman -0.8 dB/octave tilt (psychoacoustically preferred for in-room) |
| `"custom"` | User-defined slope in `slope_db_per_octave` |
| `"file"` | Load target from CSV (`curve_path`) |
| `"from_measurement"` | Auto-derive slope from input measurement |

Add bass/treble preference shelves:
```json
{
  "target_response": {
    "shape": "harman",
    "preference": {
      "bass_shelf_db": 3.0,
      "bass_shelf_freq": 200
    }
  }
}
```

## Advanced Features

### Excursion Protection (Bookshelf Speakers)

Automatically detects F3 rolloff and generates a highpass filter to prevent driver damage:

```json
{
  "optimizer": {
    "excursion_protection": {
      "enabled": true,
      "auto_detect_f3": true,
      "filter_order": 4
    }
  }
}
```

### Schroeder Frequency Split

Applies high-Q narrow filters below Schroeder (room modes) and low-Q broad filters above:

```json
{
  "optimizer": {
    "schroeder_split": {
      "enabled": true,
      "room_dimensions": { "length": 5.0, "width": 4.0, "height": 2.5 }
    }
  }
}
```

### Phase Alignment (Sub Integration)

Optimizes subwoofer delay and polarity for maximum energy sum at crossover:

```json
{
  "optimizer": {
    "phase_alignment": { "enabled": true, "min_freq": 60, "max_freq": 100 }
  }
}
```

### Multi-Seat Optimization

Minimizes variance across multiple listening positions:

```json
{
  "optimizer": {
    "multi_seat": { "enabled": true, "strategy": "minimize_variance" }
  }
}
```

### FIR / Mixed Mode

Generate linear-phase or mixed IIR+FIR correction filters:

```json
{
  "optimizer": {
    "mode": "mixed",
    "fir": { "taps": 4096, "phase": "kirkeby" },
    "mixed_config": { "crossover_freq": 300, "fir_band": "low" }
  }
}
```

## Output Format

The output JSON contains per-channel DSP chains compatible with SotF's audio engine:

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
    "algorithm": "autoeq:de"
  }
}
```

## Validation

Validate a config without running optimization:

```bash
roomeq --config room.json --output /dev/null --dry-run
```

Inspect the full input schema:

```bash
roomeq --schema input
```

## Logging

`roomeq` uses the standard `RUST_LOG` environment variable:

```bash
RUST_LOG=debug roomeq --config room.json --output out.json
RUST_LOG=warn  roomeq --config room.json --output out.json   # quiet
```

## Override Config

Apply optimizer setting overrides without editing the main config (useful for scripting):

```bash
cat > quick.json <<'EOF'
{ "optimizer": { "num_filters": 5, "max_iter": 5000 } }
EOF

roomeq --config room.json --output room-quick.json --override-config quick.json
```
