# HAL Output plugin review — 2026-08-12

## Findings

### P0 — HAL writer returns frames, but the plugin interprets the count as samples

**Fixed in 0.5.10.** The plugin now names and validates the writer result as
frames, compares it with `context.num_frames`, and computes frame-based success.
An injectable writer test covers exact full writes at 1, 2, 6, and 16 channels.

`HalOutputWriter::write` forwards `SharedAudioBuffer::write_audio`, whose return value is `frames_written = samples_written / channel_count`; encrypted writes also return frames (`driver-hal/.../shared_audio_buffer.rs:974-1031,1331-1430`; `hal_output_writer.rs:71-103`). The plugin compares that frame count with `input.len()` samples, computes a sample-based percentage, and divides it by channels again for logging (`lib.rs:230-257`). A completely successful stereo callback is therefore reported as 50% success/backpressured and counted as an underrun; six channels report about 16.7%. The diagnostics added in the latest changelog are systematically wrong for every multichannel stream.

Make the return unit type-safe or name it `frames_written`, compare with `context.num_frames`, and remove the extra channel division. Audit HAL Input simultaneously: it has the inverse bug with destructive zero-fill. Add exact full/partial write assertions for 1, 2, 6, and 16 channels on both plain and encrypted paths.

### P1 — Partial writes irreversibly drop audio while the plugin reports full consumption

**Fixed at the plugin boundary in 0.5.10.** A preallocated frame-aligned pending
queue retains unwritten tails and retries them before new input. The ordering
test verifies sequence preservation across a partial write and recovery.
**Completed in 0.5.11.** The queue is now a non-compacting bounded FIFO with a
documented drop-newest policy and loss telemetry. Saturation/recovery tests
verify complete-frame ordering. Plain shared-memory writes now round available
space down to complete interleaved frames, including at wrap boundaries.

When the shared ring accepts fewer frames, the unwritten tail is discarded; there is no pending-output queue or retry (`lib.rs:230-263`). Nevertheless `process` always returns `context.num_frames`, telling the host the whole block was consumed. Sustained backpressure therefore creates silent gaps/data loss, and the diagnostic counter cannot recover content. The underlying plain writer may also commit a sample count not divisible by channel count before flooring its frame return (`shared_audio_buffer.rs:974-1031`), which can break interleaved frame alignment.

Define sink backpressure semantics. Prefer a preallocated frame-aligned pending ring sized from the graph contract, retry queued frames before new input, and expose deterministic overflow policy; alternatively allow partial consumption in the host API. Fix the shared writer to transact complete frames only. Add long-running producer/consumer rate mismatch, ring-near-full, wrap, partial-frame-capacity, and recovery tests with sequence-numbered audio.

### P1 — The plugin never negotiates or validates sample rate/channel format

**Fixed in 0.5.10.** Initialization negotiates exact rate/channel format,
processing rejects runtime rate mismatch, and transport configuration-change
notifications trigger revalidation before another write.

`HalOutputPlugin` has no `initialize` override. Construction accepts an independent 1–16 channel count but reads neither `writer.sample_rate()` nor `writer.channel_count()` (`lib.rs:87-121`), even though `HalOutputWriter` exposes both and `current_format`. Audio can be written with a different rate or layout than the shared header, causing pitch/duration errors or channel/frame misalignment. Unlike HAL Input, this endpoint does not fail fast.

Validate exact sample rate and channel layout during initialization and on every format generation change, or explicitly reconfigure the transport on a non-realtime control thread with an acknowledged handshake. Add mismatched rate/channel and live format-change integration tests.

### P1 — Reported latency is ring capacity, not output latency

**Fixed conservatively in 0.5.10.** Ring capacity is no longer reported as
graph latency; the plugin reports zero until a measured target-fill/device
latency contract exists.
**Completed in 0.5.12.** Initialization and every control-thread re-service
flush and prime the shared ring to exactly one negotiated HAL buffer before
readiness is published. Fixed boundary latency is now the target fill plus the
Swift virtual-device latency and safety offset. Typed v2 telemetry reports each
component and observed fill separately. Priming failure is transactional: the
ring is flushed and readiness remains false.

The plugin caches `buffer_frames` and reports it as latency (`lib.rs:99-105,229,267-277`). Ring capacity is a maximum, while actual playout delay depends on fill level, HAL safety/device latency, callback phase, reader scheduling, and encryption records. Returning capacity overcompensates the graph; updating it inside processing can also change latency without graph recompilation.

Maintain a defined target fill and report a fixed measured boundary latency including device offsets, or leave graph latency unknown and expose capacity/fill separately. Add timestamped loopback/impulse tests across callback sizes and load.

### P1 — Lifecycle, readiness, reconnect, and cipher rotation hooks are unused

**Partially fixed in 0.5.10.** Initialization/drop now assert/deassert engine
readiness and format changes are acknowledged after validation. Daemon remap,
writer replacement, and key rotation require a non-realtime supervisor API and
remain deferred; no filesystem/key reload was moved into the callback.
**Completed in 0.5.11.** `service_transport()` is the explicit control-thread
supervisor entry point: it quiesces readiness, reconnects/replaces the mapping,
reloads the cipher, validates format, acknowledges configuration, and reasserts
readiness. The callback queues while disconnected, key-mismatched, or awaiting
configuration service and performs no filesystem/key I/O. Lifecycle state and
tests distinguish these causes.

The writer API exposes `set_engine_ready`, configuration-change flags, `reload_cipher`, and format setters/getters, but the plugin only calls `new`, `is_connected`, `buffer_frames`, and `write`. It never marks engine readiness, handles daemon/shared-memory replacement, responds to configuration generation changes, or reloads a rotated encryption key. `HalOutputWriter::write` returns zero indefinitely on fingerprint mismatch (`hal_output_writer.rs:71-103`), which this plugin mislabels as backpressure.

Implement a non-realtime lifecycle state machine that negotiates format, readiness, key reload, reconnection, and atomic writer replacement. Distinguish not-ready, disconnected, key mismatch, full ring, and fatal format error in telemetry. Test daemon restart, driver reload, key rotation, and device switch.

### P1 — Warning logging occurs on the realtime sink path

**Fixed in 0.5.10.** Callback logging was removed; partial writes update relaxed
atomic/cached telemetry only.

Partial writes call `log::warn!` on the first event and every 1000 counted events (`lib.rs:242-259`). Rate limiting reduces volume but not worst-case lock, allocation, formatting, timestamp, or I/O latency; due the unit bug, every multichannel callback also advances the counter.

Record lock-free event counters/codes on the callback and emit logs from a control thread. Test callback tail latency and allocation with warning logging enabled.

### P1 — HAL-enabled production behavior has no crate-level tests

**Fixed for plugin behavior in 0.5.10.** The writer is injected behind a small
trait, allowing platform-independent production-branch tests for full,
partial, invalid, and format-sensitive writes. Encrypted/shared-memory soak
coverage remains owned by `driver-hal` integration tests. **Completed in
0.5.11:** driver production-ring coverage includes partial-frame/wrap behavior;
plugin tests add queue saturation/recovery, lifecycle/configuration service,
maximum-channel allocation freedom, and invalid-writer recovery.

All tests build a private `writer: None` struct and exercise only validation/metadata; none calls the successful writer branch (`lib.rs:282-408`). TokenSave reports 0% graph coverage for production entry points. There is no QA binary, shared-memory integration test, allocation test, or soak test, so the frame/sample bug survived despite 16 passing tests.

Introduce an injectable writer trait/fake for cross-platform tests plus macOS feature-gated shared-memory tests. Cover full/partial/zero writes, encryption, wrap, format mismatch, reconnect, diagnostics, and allocation freedom.

### P2 — Diagnostics are mislabeled and lose important information

**Partially fixed in 0.5.10.** User-facing text now calls these backpressure
events and the legacy `underrun_count` value saturates at `i32::MAX` rather
than wrapping. **Completed in 0.5.11.** Diagnostics no longer masquerade as
automatable parameters. `HalOutputTelemetry` is versioned and preserves 64-bit
requested/written/dropped/event counters plus queued/capacity frames,
connection/key readiness, and a typed cause state. A regression test exercises
counters beyond `i32::MAX`.

The counter field/comment calls partial writes “underruns,” although a full producer-side ring is normally overrun/backpressure; parameter text also says underrun (`lib.rs:58-66,156-190`). The `u64` count is cast to `i32` and eventually wraps. `write_success_ratio` is only the latest block and, even after fixing units, does not report frames dropped, pending, fill, capacity, or cause. Connection state is sampled only while processing.

Use typed transport telemetry: overrun/backpressure events, dropped/queued frames, fill/capacity, connection/config generation, key status, last error, and lifetime success ratio, preserving 64-bit counts. Keep diagnostics outside automatable parameter metadata.

### P2 — Size arithmetic and transport return values are unchecked

**Fixed in 0.5.10.** Frame/sample sizing uses checked multiplication and writer
results larger than the request are rejected. Tests cover overflow-shaped
contexts and the injectable writer contract.

`num_frames * channels` can overflow (`lib.rs:218-227`). The writer return is not checked against the requested frames or for channel alignment before percentage/log arithmetic. A broken transport contract can produce nonsensical ratios or hide corruption.

Use checked multiplication and validate the typed returned frame count. Add overflow-shaped contexts and fake-writer invalid-return tests.

### P2 — Sink output-buffer contract is not validated

**Fixed in 0.5.10.** Nonempty output is rejected explicitly for this zero-output
sink, with regression coverage.

`output_channels()` is zero, but `process` ignores `_output` regardless of its length (`lib.rs:148-150,212-216`). If the host incorrectly allocates/routes a nonempty downstream buffer, the wiring error is silently accepted.

Require an empty output slice for sink nodes or document a host-wide convention that ignored output is legal. Add contract tests for nonempty output.

### P3 — Configuration and registration cannot round-trip channel layout

**Fixed in 0.5.11.** Canonical serializable `params::Params` persists the
construction-only `channels` layout while keeping the automatable parameter
schema empty. The legacy `output_channels` spelling migrates via a serde alias;
default, migration, and non-stereo round trips are tested. Factory construction
uses this canonical type.

The public constructor config has `channels`, but canonical `params::Params` is empty and old `output_channels` fields are intentionally ignored (`lib.rs:39-49`; `params.rs:38-110`). Catalog/factory wiring must therefore carry layout out of band; a serialized standalone plugin cannot reconstruct it from the canonical parameter state. Diagnostics are returned by the runtime `parameters()` list but absent from `ParamSpec`, creating two schemas.

Define one structural node configuration containing the channel layout and a separate telemetry schema. Add JSON→factory→compiled-layout round-trip tests for every supported channel count.

## Algorithm assessment

This sink's algorithm is a real-time transport protocol: format negotiation, complete-frame transactions, bounded backpressure buffering, lifecycle recovery, encryption/key state, readiness, and latency/fill control. Those contracts are more important than sample arithmetic. The current direct write is simple, but it needs complete-frame and retry semantics before it can deliver glitch-free system-wide output.

## Real-time allocation and performance assessment

The plugin adds no normal scratch allocation and uses relaxed atomic telemetry.
In 0.5.11 the pending queue no longer compacts or moves its full capacity; a
maximum-channel test covers full, queued, and recovery callbacks with the
counting allocator armed. The underlying encrypted writer preallocates buffers
and rejects over-capacity work. Reconnect and key reload remain explicitly
outside the realtime path.

## Scope reviewed

Read in full: `AGENTS.md`, `CHANGELOG.md`, `Cargo.toml`, `README.md`, all of `src/lib.rs` including inline tests, and `src/params.rs` including tests. Relevant wiring reviewed includes factory/catalog construction, `Plugin` boundary metadata, `HalOutputWriter::{new,write,current_format,sample_rate,channel_count,buffer_frames,is_connected,set_engine_ready,config_changed,reload_cipher}`, plain/encrypted `SharedAudioBuffer` write implementations, shared-memory ring behavior, systemwide daemon/HAL ownership, and host sink/channel/latency contracts. No production code was changed.

## Strengths

- Invalid channel counts and unavailable platform/feature/daemon construction fail explicitly.
- Input buffer size mismatch returns an error.
- Static hot-path error strings and relaxed atomic counters avoid some incidental work.
- Diagnostics expose connection and recent backpressure state.
- The writer's encryption scratch is prepared during construction and capacity failures are bounded rather than growing indiscriminately.

## Verification

`rtk cargo test -p sotf-plugin-hal-output` — 16 tests passed across two suites (metadata/non-HAL paths only).
