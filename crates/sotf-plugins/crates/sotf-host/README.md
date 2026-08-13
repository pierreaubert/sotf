# sotf-host

Core traits, host, and shared utilities for SOTF audio plugins.

Foundation crate for the SOTF plugin system providing:
- Plugin and analyzer traits
- Parameter system with automation support
- Plugin host for chaining processors
- Smoothing utilities for click-free parameter changes
- SIMD helpers for audio processing

## Hosting external plugins

Native hosting is opt-in: enable `external-plugin-clap`,
`external-plugin-vst3`, or (on macOS) `external-plugin-au`. A disabled or
failed backend is a graph-construction error; SOTF never silently substitutes
dry passthrough for a requested native processor.

`PluginScanner` performs filesystem discovery only. Its descriptors use zero
channels to mean "metadata not probed" and must not be used to size isolated
IPC. In-process loading replaces discovery metadata with the native ABI
metadata. Isolated callers must supply probed channel metadata.

Native instances allocate planar scratch for their negotiated maximum block
size. `ExternalPlugin::new_with_max_block_frames` exposes that contract;
isolated workers use the maximum from their shared-memory layout. The legacy
constructor uses an 8192-frame upper bound.

Isolated processing keeps shared memory mapped for the worker lifetime and
preallocates all block scratch. A wait deadline is capped at 75% of the current
block period. Runtime worker failures use dry audio delayed by the worker's
reported latency and a bounded 64-frame transition; startup failures abort
graph construction because latency is not yet known. The sandbox is the
process boundary—the mapping is not a security boundary against code already
running inside the worker.

The version-2 isolated protocol has bounded MIDI and parameter-event rings plus
a separate 64 KiB non-realtime control region. It exposes parameter discovery,
set/get, and native save/load state; `capture_worker_state` refreshes the
restart sidecar before project save. MIDI and parameter sample offsets and
transport metadata survive the process boundary. Oversized event/control
payloads fail explicitly rather than allocating on the audio thread.
