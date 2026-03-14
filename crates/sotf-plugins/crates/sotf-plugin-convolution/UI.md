# Convolution — UI Specification

## Layout Mode
custom

## Menu Bar

| Position | Element | Behavior |
|----------|---------|----------|
| Left | "Convolution" label | Plugin name |
| Right | Preset picker | Standard preset dropdown |
| Right | T S X | Toggle bypass / Solo / Close (remove plugin) |

## Layout

```
+------------------+--------------------------------------------+------------------+
| SETUP            | (IR waveform display placeholder)           | OUTPUT           |
|                  |                                            |                  |
| [∿] IR Loaded    | "IR Waveform" or "No IR loaded"            | [Mix]      knob  |
|   filename.wav   |                                            | [Gain]     knob  |
|   [Load]  btn    |                                            |                  |
+------------------+--------------------------------------------+------------------+
```

## Config (Left Column — "SETUP")

Width: 180px fixed

| Element | Control | Param Index | Notes |
|---------|---------|-------------|-------|
| IR Status Icon | Icon (∿) | — | Colored: accent if loaded, surface if empty |
| IR Status Label | Text | — | "IR Loaded" or "No IR File" |
| Filename | Text (truncated) | — | Shows basename of loaded file, or "Load an IR file" |
| Load Button | Button | — | Dispatches `OpenIrFile` action to open file picker |

The IR file parameter (engine_key: `ir_file`) is set via file dialog, not directly editable.

## Main (Center — IR waveform)

| Element | Type | Notes |
|---------|------|-------|
| IR Waveform | Placeholder panel | Shows "IR Waveform" when loaded, "No IR loaded" otherwise. 60px height. Future: actual waveform rendering |

## Output (Right Column)

Width: 120px fixed

| Parameter | engine_key | Control | Param Index | Shortcut | Notes |
|-----------|-----------|---------|-------------|----------|-------|
| Mix | mix | Knob | 1 | m | 0–1.0, display as %. Dry/wet blend |
| Gain | gain_db | Knob | 2 | g | -20 to +20 dB |

Note: Param index 0 is the IR file path (not rendered as a knob).

## Visualizations

### IR Waveform (future)
- **Type:** Waveform display of loaded impulse response
- **Size:** Full center width, 60px height
- **Content:** Time-domain amplitude of the IR

## Responsive Behavior
- **Compact:** Center column hidden, setup and output stack vertically
- **Wide:** 3-column layout as shown

## ParamCategory Mapping

| Parameter | engine_key | Category | Group |
|-----------|-----------|----------|-------|
| IR File | ir_file | Setup | General |
| Mix | mix | Output | General |
| Gain | gain_db | Output | General |
