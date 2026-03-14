# Spectrum Analyzer — UI Specification

## Layout Mode
custom

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "Spectrum" label | Plugin name |
| Right | T S X | Toggle bypass / Solo / Close (remove plugin) |

## Layout

```
+---------------------------------------------------------------------+
| menu Spectrum                                            | T S X    |
+---------------------------------------------------------------------+
| ┌─ dB ─┬─ SPECTRUM DISPLAY ──────────────────────────────────────┐  |
| │ +3   │                                                          │  |
| │  0   │  ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓         │  |
| │ -20  │  ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓         │  |
| │ -40  │  ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓         │  |
| │ -60  │  ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓▓ ▓▓▓         │  |
| └──────┴─────────────────────────────────────────────────────────┘  |
|          20  50  100  200  500  1k  2k  5k  10k  20k               |
+---------------------------------------------------------------------+
| CONFIG                                                              |
| [Bins] knob  [Min Hz] knob  [Max Hz] knob  [Smooth] knob           |
| [Tilt] selector  [Reference] selector                               |
+---------------------------------------------------------------------+
```

## Main — Spectrum Display (top, full width)

### Spectrum Graph
- **Type:** GPU-accelerated bar spectrum using SpectrumElement
- **Size:** Full width, 200px height (configurable)
- **dB Axis:** Left side, labels at +3, 0, -20, -40, -60 dB
- **Frequency Axis:** Bottom, logarithmic scale, labels at standard frequencies
- **Bar coloring:** Level-based (green < -6 dB, yellow -6 to -1 dB, red > -1 dB)
- **Data source:** RealTimeCache<SpectrumData> via `get_data()`

## Config (bottom, horizontal row with wrapping)

| Parameter | engine_key | Control | Param Index | Notes |
|-----------|-----------|---------|-------------|-------|
| Bins | num_bins | Knob (integer) | 0 | 10–100 |
| Min Hz | min_freq | Knob | 1 | 10–1000 Hz |
| Max Hz | max_freq | Knob | 2 | 1000–22050 Hz |
| Smoothing | smoothing | Knob | 3 | 0–1.0 |
| Tilt Correction | tilt_correction | Select dropdown | 4 | None / 3dB/oct / 6dB/oct / Pink |
| Tilt Reference | tilt_reference | Select dropdown | 5 | Standard / 1kHz / 2kHz / Min Freq |

## Visualizations

### Spectrum Bars
- **Type:** Real-time frequency spectrum bars
- **Size:** Full width, 200px height
- **Color scheme:** Green (safe) → Yellow (caution) → Red (clipping) based on dB level
- **Smoothing:** Temporal smoothing applied per frame using configurable factor
- **GPU-accelerated:** Direct quad rendering via SpectrumElement for maximum performance

## Responsive Behavior
- **Compact:** Config controls wrap to multiple rows, spectrum display shrinks to 120px height
- **Wide:** All config controls in single row, spectrum at full 200px height

## ParamCategory Mapping

| Parameter | engine_key | Category | Group |
|-----------|-----------|----------|-------|
| Bins | num_bins | Setup | General |
| Min Freq | min_freq | Setup | General |
| Max Freq | max_freq | Setup | General |
| Smoothing | smoothing | Setup | General |
| Tilt Correction | tilt_correction | Setup | General |
| Tilt Reference | tilt_reference | Setup | General |
