# sotf-plugin-hal-input

SOTF HAL Input plugin for macOS audio HAL input.

Reads audio data from the macOS CoreAudio HAL driver via shared memory, acting as the input source for system-wide audio processing.

`input_channels` is structural and must match the negotiated HAL stream. Runtime
status is available through `HalInputPlugin::diagnostics()` rather than plugin
automation. When `needs_control_recovery` becomes true, a control thread may call
`refresh_transport()`; that operation can open mappings and load keys and must
never run in the audio callback. The plugin reports zero graph-compensation
latency because shared-memory capacity is not measured signal latency.
