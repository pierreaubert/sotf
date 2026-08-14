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
key-loading work. Re-service quiesces readiness once, discards stale queued
audio, flushes the shared ring, and re-primes it before publishing readiness.
A failed prime leaves readiness false and the ring empty.

Initialization maintains a target shared-ring fill equal to the negotiated HAL
buffer size. `latency_samples()` reports that deterministic transport delay plus
the Swift virtual device latency (one negotiated device buffer) and its
zero-frame safety offset. `HalOutputTelemetry` v2 exposes target and observed
fill, device latency, safety offset, and their fixed boundary-latency sum.
