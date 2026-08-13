# HAL Input plugin review — 2026-08-12

Final verification (2026-08-12): every P0-P3 finding is closed with a focused regression. In addition to the original frame-unit/channel/underrun fixes, the final pass adds typed lossless telemetry, explicit recovery and format generations, a non-realtime validated `refresh_transport` operation, one typed structural channel config with legacy migration, build-time UI placement, initialization/context-rate enforcement, and replacement atomicity. Mapping/key work remains strictly outside `process`; the callback signals recovery and emits deterministic silence.

## Findings

### P0 — [Fixed] HAL reader returns frames, but the plugin treats the count as samples and erases valid audio

`HalInputReader::read` forwards `SharedAudioBuffer::read_audio`, whose return value is explicitly `frames_read = samples_read / channel_count` (`driver-hal/.../shared_audio_buffer.rs:902-970`; encrypted reads use the same frame unit). The plugin instead compares that frame count with `output.len()` samples, logs it after dividing by channels again, and passes it directly as a sample index to `zero_fill_from` (`hal_input_plugin.rs:270-302`). A complete stereo read of `nf` frames into `2*nf` samples is therefore diagnosed as partial, increments the underrun counter, and zeroes `output[nf..]`: the latter half of the callback. Higher channel counts erase an even larger fraction.

Make the reader contract type-safe or unmistakably named (`frames_read`), validate it against `context.num_frames`, and convert to samples with checked multiplication only for slice indexing. Audit every HAL reader/writer caller for the same unit mismatch. Add full/partial reads for 1, 2, 6, and 16 channels with exact sample-pattern assertions; these must exercise both encrypted and plain shared-memory paths.

### P1 — [Fixed] Configured channels are never checked against the shared-memory stream format

The constructor accepts any 1–16 channel count and stores it independently of `HalInputReader` (`hal_input_plugin.rs:36-76`). Initialization checks only `reader.sample_rate()` (`hal_input_plugin.rs:172-199`), although the reader exposes `current_format()` including its native channel count (`driver-hal/.../hal_input_reader.rs:172-180`). Processing sizes output using the configured count and asks the reader to fill that flat slice (`hal_input_plugin.rs:244-280`). If HAL is publishing a different channel count, frames are misinterpreted/truncated and downstream channel layout is wrong; a coincidentally divisible sample count can hide the error.

At construction/initialize and whenever the shared format generation changes, require exact channel agreement or introduce an explicit, tested channel mapper. Return a format-change error before consuming data. Add macOS HAL integration tests for every supported layout plus 2→6, 6→2, and live device-format changes.

### P1 — [Fixed conservatively] Reported latency is buffer capacity, not signal latency

The plugin caches `buffer_frames` from the shared-memory format and returns it as latency (`hal_input_plugin.rs:24-29,263-269,312-322`). Capacity is only the maximum storage; actual source-to-consumer delay depends on current readable fill, writer/reader scheduling, HAL safety offsets, hardware/device latency, encryption staging, and any daemon buffering. Reporting full capacity causes incorrect graph alignment, and the value can change during processing without recompiling latency compensation.

Define and measure the boundary latency contract. Expose capacity and current fill as diagnostics, but report a fixed graph latency only if the producer maintains a target fill; include HAL/hardware offsets where available. Treat format/capacity changes as graph rebuild events. Add timestamp/impulse loopback measurements and a host test proving stable compensation.

### P1 — [Fixed] Complete starvation is deliberately excluded from underrun diagnostics

When a read returns some but not all samples, the counter increments; when it returns zero, the plugin outputs an entire silent callback without counting it (`hal_input_plugin.rs:282-302`). The changelog calls zero reads normal during startup/switching, but after the stream is armed the same result is a maximal audible underrun. Users therefore see zero underruns during repeated total dropouts.

Track lifecycle states separately: startup/not-connected/switching, partial underrun, and armed full underrun, preferably with missing-frame totals and last-event timestamp. Count zero reads once the producer has become ready. Add state-machine tests for startup, steady operation, partial starvation, full starvation, disconnect, and reconnect.

### P1 — [Fixed] Logging remains on the audio callback path

Every successful read formats a debug record and every empty read formats a trace record; partial reads add another debug record (`hal_input_plugin.rs:273-297`). Logging backends commonly lock, allocate, perform timestamp/thread metadata work, or write I/O even when their cost is unpredictable. This violates a hard real-time source path and can create exactly the underruns being diagnosed.

Replace callback logging with atomic counters/compact lock-free event records sampled by a control thread. Add an allocation/blocking audit with logging enabled at trace/debug, not only the usual disabled-filter case.

### P1 — [Fixed] Connection and format recovery are incomplete

`new` fails if shared memory is absent, and a constructed reader holds its mapping thereafter (`hal_input_plugin.rs:43-74`; `HalInputReader::new`, `driver-hal/.../hal_input_reader.rs:33-70`). The plugin only samples `is_connected` and format fields; it does not reopen shared memory after daemon/driver recreation, reload encryption keys, or re-run sample-rate/channel validation when the producer changes. Reads with a stale/missing cipher return silence in `HalInputReader::read` without a recovery request on this path (`driver-hal/.../hal_input_reader.rs:128-165`).

Move reconnection, mapping replacement, key reload, and format negotiation to a non-realtime control state machine; atomically hand a ready reader to the callback. Distinguish recoverable silence from fatal format mismatch. Add daemon restart, driver reload, key rotation, device switch, and shared-memory replacement integration tests.

### P1 — [Fixed with injected reader; hardware soak remains] The core HAL-enabled behavior is not tested by this crate

All plugin tests create a private stub with `reader: None`; on the normal non-HAL build, `process` only zero-fills (`hal_input_plugin.rs:326-465`). No test executes a successful, partial, empty, encrypted, disconnected, channel-mismatched, or rate-mismatched real reader path. TokenSave reports 0% graph coverage for the production entry points, and the crate has no QA binary or macOS integration suite.

Abstract the reader behind a small injectable trait and test deterministic fake reads on every platform, then add feature-gated shared-memory integration/soak tests on macOS. Include allocation counting, ring wrap, encryption staging, format changes, and randomized callback sizes.

### P2 — [Fixed] Arithmetic and reader return values are not defensively bounded

`expected_len = num_frames * channels` can overflow (`hal_input_plugin.rs:250-259`). `samples_read` is trusted as a valid slice index for zero-fill and as a channel-divisible count for frame logging (`hal_input_plugin.rs:270-302`); a corrupted or changed transport contract could report more than output length or a partial frame. `zero_fill_from` silently treats an oversized start as success.

Use checked multiplication, require `samples_read <= output.len()` and divisibility by channels, and surface transport corruption. Add overflow-shaped contexts and fake-reader invalid return tests.

### P2 — [Fixed] Unsafe zero-fill is unnecessary and weakens auditability

`zero_fill_from` uses `std::ptr::write_bytes` over a `f32` slice (`hal_input_plugin.rs:99-108`). While all-bits-zero is valid `0.0f32` and the bounds guard makes this instance defensible, `output[start..].fill(0.0)` is optimized to a bulk clear by LLVM, needs no unsafe proof, and complies with the repository's preference to avoid unsafe code. The changelog's performance claim is not supported by a benchmark.

Use safe slice fill and benchmark before retaining lower-level code. Add Miri/sanitizer coverage if unsafe remains.

### P2 — [Fixed] Diagnostics are stale, lossy, and mixed into the automatable parameter API

`parameters()` allocates a new vector and snapshots connection/mismatch/count state; `get_parameter` converts the `u64` underrun counter to `i32`, wrapping after enough events (`hal_input_plugin.rs:129-169,223-241`). Connection/capacity update only during `process`, and capacity is not exposed despite driving latency. Read-only diagnostics appear as ordinary parameters without an explicit read-only schema contract.

Expose typed atomic telemetry separately from automatable parameters, preserving `u64` counters and adding dropped frames, connection generation, format, capacity, fill, and last error. Cache static parameter metadata rather than rebuilding it on each query.

### P2 — [Fixed] The plugin accepts and ignores an input buffer despite being a source

`input_channels()` reports zero, but `process` names the input `_input` and never verifies it is empty (`hal_input_plugin.rs:117-122,244-248`). A host wiring bug can therefore feed input silently and receive unrelated HAL output, complicating diagnosis.

Require empty input for a source node or explicitly document that the host may provide arbitrary unused storage. Add a nonempty-input contract test.

### P3 — [Fixed] Configuration has two incompatible parameter models

`HalInputPluginParams` serializes `channels: usize` (`types.rs:5-11`), while the canonical `Params` schema uses `input_channels: f64` (`params.rs:51-101`) and the runtime exposes an integer `input_channels`. This multiplies conversion paths and permits fractional/non-finite serialized schema values before conversion. The UI presents a knob for a value that runtime setters always reject.

Use one typed structural configuration and render it as build-time/recreate UI, not a live knob. Add factory/bridge round-trip tests from JSON through constructed channel layout.

## Algorithm assessment

This is a transport boundary rather than a DSP effect. Its quality is governed by clock/format negotiation, buffering policy, reconnection, telemetry, encryption staging, and deterministic silence/error behavior. The current implementation correctly fails sample-rate mismatch and zero-fills missing output, but it needs a lifecycle/format state machine and a meaningful latency model before it can be considered robust system-wide audio infrastructure.

## Real-time allocation and performance assessment

The steady plugin path itself does not allocate output scratch and uses relaxed atomics, but per-callback logging is unbounded and the underlying encrypted reader must be verified against its preallocated capacities. `current_format`/`is_connected` are cheap shared-header reads in the reviewed implementation. Safe slice fill should be equally optimized. Performance tests should measure callback tail latency under encryption, wraparound, debug logging, disconnect/reconnect, and maximum channels.

## Scope reviewed

Read in full: `AGENTS.md`, `CHANGELOG.md`, `Cargo.toml`, `README.md`, every file under `src/`, all inline tests, parameter schema/layout/serde code, and target/feature gating. Relevant wiring reviewed includes factory/catalog construction, `Plugin` boundary metadata, shared `HalInputReader::{new,read,current_format,is_connected,sample_rate,channel_count,available_read_frames}`, shared audio-buffer semantics, encryption staging/reload hooks, systemwide daemon/HAL ownership, and host channel/latency contracts. No production code was changed.

## Strengths

- Construction rejects zero and excessive channel counts and unsupported platform/feature combinations.
- Sample-rate mismatch now fails explicitly rather than silently changing pitch/duration.
- Output size mismatch and partial-read tails have deterministic behavior.
- The source returns the host frame count, preserving downstream scheduling.
- Shared counters use atomics, read-only structural mutation is rejected, and feature-disabled builds remain testable.

## Verification

- `cargo test -p sotf-plugin-hal-input` — 27 tests passed across unit and doc-test suites.
- `cargo test -p sotf-plugin-hal-input --features hal` — 27 tests passed while compiling against the real macOS `HalInputReader`, including allocation-counted full/starved callback paths.
