# sotf-plugin-hal-output

SOTF HAL Output plugin for macOS audio HAL output.

Writes processed audio data back to the macOS CoreAudio HAL driver via shared memory, completing the system-wide audio processing chain.

The serialized construction state is `{ "channels": N }` for layouts from 1
through 16 channels. The legacy `output_channels` key is accepted when loading
presets. Channel layout is construction-only and is not exposed as an
automatable plugin parameter.

`process()` performs only bounded, allocation-free transport work. Partial
writes are retained in a preallocated complete-frame FIFO. When the FIFO is
full, the newest complete frames are dropped deterministically and counted in
`HalOutputTelemetry`; older queued audio always keeps its order. Transport
diagnostics are available through `telemetry()` as versioned 64-bit counters,
not through the automation parameter schema.

Daemon restarts, shared-memory remapping, configuration changes, and encryption
key rotation are serviced by `service_transport()`. Hosts must call it from a
non-realtime control thread; the audio callback never performs filesystem or
key-loading work. A configuration-change notification quiesces writes and
queues input until servicing completes.

`latency_samples()` is zero because ring capacity is not a known compensable
playout delay. Capacity and queued frames are reported separately in telemetry.
