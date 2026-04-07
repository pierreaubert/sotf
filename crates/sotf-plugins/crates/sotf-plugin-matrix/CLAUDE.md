# sotf-plugin-matrix

Channel matrix mixer plugin routing N input channels to P output channels with gain coefficients, phase inversion, and routing presets.

## Architecture

```
src/
  lib.rs    -- MatrixPlugin (Plugin), routing presets, identity matrix construction
  params.rs -- Centralized parameter specs
```

Data flow: Input channels -> channel mapping (input_channel_map/output_channel_map) -> gain matrix with smoothing -> optional phase inversion per connection -> channel mute/solo/dim states -> output channels.

**Key types:**

- `MatrixPlugin` -- Main plugin implementing `Plugin` (variable I/O channel counts). Holds a flat gain matrix `matrix[out * input_channels + in]`, per-connection phase inversion flags, and per-output channel mute/solo states via `ChannelState` from sotf-plugin-channel-mute-solo.
- Gain smoothers (5ms) prevent clicks on matrix coefficient changes.

**Routing presets:**

- `custom` -- User-defined matrix
- `stereo_downmix` -- N channels to stereo
- `ms_encode` / `ms_decode` -- Mid/Side encoding/decoding
- `5.1_remap` -- 5.1 channel remapping

## Key Public API

- `MatrixPlugin::new(input_channels, output_channels) -> Self` -- Identity matrix (`lib.rs`)
- Implements `Plugin` trait (input_channels can differ from output_channels)

**Parameters:** `preset` (choice), `matrix` (JSON array of gain coefficients), `phase_invert` (JSON array of bools), `gain` (global linear gain 0-1), per-channel mute/solo/dim via `mute_{N}`, `solo_{N}`, `dim_{N}`.

## Testing

```bash
cargo test -p sotf-plugin-matrix
```

## Important Notes

- The matrix is stored as a flat Vec: `matrix[out_ch * num_inputs + in_ch]` = gain coefficient. Negative gains produce phase inversion; the separate `phase_invert` flags provide explicit per-connection control.
- Channel maps (`input_channel_map`, `output_channel_map`) allow routing a subset of physical channels through the matrix. When empty, all physical channels are used.
- Depends on `sotf-plugin-channel-mute-solo` for `ChannelState` type, enabling per-output-channel mute/solo/dim with smoothed transitions.
- Active connections are pre-computed and cached (`active_connections`) to skip zero-gain entries during processing.
- Default construction creates an identity matrix (1:1 pass-through for min(in, out) channels).
