# External audio-plugin adapter review — 2026-08-12

## Remediation status — 2026-08-12

All P1–P3 findings in this review are remediated on the feature branch:

### Final focused-test closure — sotf-host 0.5.100

- Realtime ownership and bounded native storage are locked by
  `warmed_success_and_timeout_paths_allocate_nothing`,
  `isolated_external_plugin_quarantines_after_repeated_block_failures`, and
  `negotiated_maximum_native_block_is_allocation_free`.
- IPC offsets, values, and capacity errors are covered by
  `event_rings_preserve_offsets_values_and_reject_overflow` and
  `versioned_ipc_preserves_event_offsets_and_transport`.
- Worker discovery, set/get, audio processing, state capture, and persisted
  restart-sidecar refresh are exercised together by
  `isolated_control_state_and_audio_share_transport_without_stale_sidecar`;
  `versioned_control_protocol_round_trips_parameters_and_state` independently
  covers every control opcode.
- `deadline_percentiles_remain_bounded_under_cpu_contention` records and bounds
  p95/p99 over 256 callbacks while another CPU thread is busy.
- `au_event_and_transport_fixture_preserves_offsets_types_and_sample_time`,
  `vst3_translation_preserves_event_offset_and_transport`, and the ignored
  native AU/VST3 state-restoration fixtures cover the format ABI boundary.

- The worker retains its mapping and preallocates exact scratch; focused realtime
  allocation tests cover successful and timeout/quarantine paths.
- Host waits are capped to 75% of the current block period, the worker no longer
  sleeps 1 ms by default, and 16-frame deadline coverage verifies the bound.
- Runtime fallback is delayed by reported latency and transitions through a
  bounded 64-frame crossfade. Impulse and discontinuity tests cover alignment.
- Missing features, binaries, launch failures, and corrupt state now fail graph
  construction instead of creating runnable passthrough nodes.
- IPC version 2 adds bounded MIDI and parameter-event rings plus a separate
  non-realtime control region for parameter discovery/set/get and state save/load.
  Worker state capture refreshes the restart sidecar.
- Scanner discovery uses explicit unprobed zero-channel metadata rather than a
  stereo guess; isolated IPC rejects it until native metadata is supplied.
- CLAP, VST3, and Audio Unit receive block transport, nonzero MIDI offsets, and
  sample-offset parameter changes. CLAP lifecycle requests are recorded and
  restart requests become graph-rebuild errors.
- Native buffer planes use a negotiated maximum block size rather than 65,536
  frames per channel. Optional CLAP/VST3/AU feature builds all compile.
- Sandbox traversal coverage remains in `external_plugin_sandbox::tests`.
- Deterministic ABI fixtures cover CLAP event lists/host callbacks, VST3 event
  lists/process context, versioned IPC offsets/transport, control state roundtrip,
  state save/load persistence, deadlines, latency fallback, and steady allocation.

## Scope and verdict

This review covers the canonical in-process `ExternalPlugin`, the feature-gated CLAP, VST3, and Audio Unit native backends, descriptor scanning and persistence, the isolated worker/process/shared-memory path, sandbox policy and launch adapters, the `sotf-plugins` factory/catalog bridge, tests, and the available documentation. The focus is correctness, audio behavior, state/automation, real-time allocation, and performance. No production code was changed.

The native wrappers are careful about ABI lifetime cleanup, buffer validation, state-format validation, and panic containment. The isolated design, however, is not ready for a low-latency audio callback: it performs filesystem mapping work for every block, synchronously spins for a fixed deadline, and substitutes latency-unmatched dry audio on failure. Isolation also removes parameter control and current-state persistence. The default workspace build enables none of the three native format features (`sotf-host/Cargo.toml:37-45`); these backends are compiled only when an application explicitly enables `external-plugin-clap`, `external-plugin-vst3`, or `external-plugin-au`.

## Findings

### P1 — The isolated worker closes and remaps shared memory around every plugin call

`SecurePluginSharedMemory::process_worker_request` copies the input, drops both the mapping and file immediately before `Plugin::process`, then opens, stats, maps, and revalidates the file immediately afterward (`secure_plugin_shared_memory.rs:270-282,317-349`). This puts unmap/close/open/fstat/mmap and header validation on the worker's per-block critical path. It also makes the apparent security property misleading: hiding a Rust field does not prevent native plugin code in the same worker process from opening the known file or inspecting the address space.

Impact: repeated kernel work and page-table churn consume a material fraction of sub-2-ms block periods and directly increase deadline misses. At 48 kHz, a 64-frame callback is only 1.33 ms.

Fix: retain a permanently mapped transport in a process that never loads third-party code, and communicate with a dedicated plugin process using an event-driven, double-buffered transport. If one worker process must own both, keep the mapping alive and treat process isolation—not temporary unmapping—as the boundary. Add a syscall/allocation assertion for steady-state blocks and p95/p99 processing benchmarks at 16, 32, 64, and 128 frames.

### P1 — Same-block IPC can exceed the callback period before DSP begins

The host publishes a block and busy-spins until completion or timeout (`external_plugin_host.rs:89-129`). The default deadline is a fixed 2,000 µs (`external_plugin_isolated/consts.rs:1-5`), while an idle worker polls by sleeping 1,000 µs (`bin/external_plugin_worker.rs:41-43,133-148`). At 48 kHz the full callback period is 0.67 ms for 32 frames and 1.33 ms for 64 frames, so the configured wait alone can overrun the period; the sleep poll, scheduler wakeup, remap cost, buffer copies, and plugin DSP come on top. The audio thread also burns CPU throughout the wait.

Fix: pipeline IPC by one block and report that block as explicit graph latency; wake the worker with a futex/semaphore/event rather than sleep polling; derive deadlines from sample rate and block size with a safety margin; assign appropriate real-time scheduling where supported. Test under CPU contention and require bounded p99 deadline performance across supported block sizes.

### P1 — Failure fallback violates the plugin's declared latency and causes clicks/comb filtering

Timeout, wrong-sequence, worker-failure, quarantine, and launch-failure paths copy current dry input directly to output (`external_plugin_host.rs:106-124,132-159`; `isolated_external_plugin.rs:312-337,450-495`). Yet the graph sees the worker-reported plugin latency (`isolated_external_plugin.rs:419-432`). A transition from processed output to undelayed dry audio therefore jumps in time by the plugin latency as well as abruptly changing gain, phase, and plugin state.

Fix: preallocate a dry delay equal to the compiled latency and crossfade to it over a short bounded interval. Keep the same latency while quarantined and define a deliberate recovery policy. Add impulse, sine, and broadband tests for timeout, repeated timeout/quarantine, and worker recovery; assert temporal alignment and bounded sample discontinuity.

### P1 — Unavailable or failed in-process plugins silently become successful passthrough nodes

Disabled format features select `Passthrough` (`external_hosting_backend.rs:21-44,47-84`), and `ExternalPlugin::new` returns success with no native backend (`external_plugin.rs:66-105`). Restore failures and opaque-state load failures are likewise converted into `unavailable_placeholder` and returned as `Ok` (`external_plugin.rs:134-180`). Processing then remains dry while `restore_error()` is merely advisory. A session can therefore sound materially different without graph construction failing.

Fix: separate a non-runnable/editor placeholder from a graph `Plugin`. Compilation should fail unless the user explicitly approves bypass; expose persistent structured status to the UI and project model. Cover disabled features, missing binaries, ABI load failure, corrupt opaque state, and later binary replacement.

### P1 — Isolated hosting has no parameter control and never captures current worker state

The isolated wrapper advertises no parameters, rejects every setter, and returns no values (`isolated_external_plugin.rs:435-447`). Only the initial opaque state is written once to a JSON file and passed at launch (`isolated_external_plugin.rs:62-90,115-120,160-182`; `bin/external_plugin_worker.rs:113-121`). `placeholder_state()` later serializes that original cached byte vector, not the worker's current state. Any runtime edits performed by a native UI or plugin-side mechanism cannot be automated or reliably saved.

Fix: add a bounded parameter/event ring with sample offsets, plus a non-real-time control channel for parameter discovery and save/load state requests. Snapshot opaque state from the worker before project save and version the protocol. Add automation round-trip, UI-edit/save/reload, worker restart, and concurrent audio/control tests.

### P2 — Scanner descriptors are filename-derived stereo guesses, but they define isolated IPC layout

The scanner does not probe plugin metadata: it synthesizes ID/vendor/version/category and hard-codes two inputs and two outputs (`plugin_scanner.rs:269-299`). `IsolatedExternalPlugin::new` constructs the immutable shared-memory layout directly from those descriptor counts before the worker loads the native plugin (`isolated_external_plugin.rs:62-71`). The factory also validates graph channels against the guessed descriptor before launch (`sotf-plugins/src/factory/create.rs:483-503`). In-process loading replaces the descriptor with native metadata (`external_plugin.rs:73-93`), but isolation cannot resize its already-created layout. Mono, surround, sidechain, instrument, or dynamically configurable plugins discovered by the scanner can fail startup or be rejected incorrectly.

Fix: move native metadata probing to a quarantined scanner process and persist a fingerprinted descriptor (binary identity/mtime/hash plus format-specific class ID and bus arrangements). Perform a startup metadata handshake before allocating IPC or compiling channels. Test mono, 2→6, zero-input instrument, multi-bus, stale-cache, and bus-layout-change cases.

### P2 — Native automation is block-boundary-only, and required host callbacks are incomplete

CLAP parameter events are always timestamped at zero (`clap_backend.rs:431-464`), VST3 reports parameter points at sample offset zero (`vst3_backend.rs:180-205,722-878`), and AU applies parameters immediately with offset zero (`au_backend.rs:333-359`). `ProcessContext` automation, MIDI, transport, tempo, and time signature are not translated. CLAP's `request_restart`, `request_process`, and `request_callback` callbacks are registered but empty (`clap_backend.rs:238-240,1010-1012`). Plugins that request restart/rescan, main-thread callbacks, tail processing, or latency changes can remain stale or stop processing correctly.

Fix: define host-level timestamped automation/events and transport data, translate them per ABI, and queue main-thread lifecycle requests outside the audio callback. Propagate dynamic latency/restart notifications into a safe graph rebuild. Add fixtures that require nonzero sample offsets, host callback servicing, tail processing, and dynamic latency.

### P2 — Every native instance reserves enormous fixed buffers and scalar-transposes every block

All three backends set `MAX_FRAMES_PER_BLOCK` to 65,536 and allocate that many planar samples per input and output channel (`clap_backend.rs:32,303-304`; `vst3_backend.rs:24,632-633`; `au_backend.rs:32,209-210`). A stereo effect reserves about 1 MiB solely for these four float planes; a 12-in/12-out instance reserves about 6 MiB. Each callback then scalar-deinterleaves and re-interleaves every sample (`clap_backend.rs:498-594`; `vst3_backend.rs:786-873`; `au_backend.rs:390-438`). This is unnecessary resident memory and O(frames × channels) copy overhead around the plugin DSP.

Fix: pass the engine's negotiated maximum block size into backend construction, allocate exactly for that contract, and rebuild on structural changes. Consider a reusable optimized planar/interleaved transpose with SIMD/batched loops. Benchmark 2, 8, and 12 channels at 32–2048 frames and track memory per instance.

### P2 — Sandbox path grants use lexical containment for authorization

`PluginSandboxPermission::satisfies` accepts read/write requests with `requested.starts_with(granted)` (`plugin_sandbox_permission.rs:15-24`) without canonicalizing either path. A request such as `/granted/../protected/file` is lexically under `/granted` but resolves elsewhere. Grant-store checks directly call this method (`plugin_sandbox_grant_store.rs:38-45`). Other policy helpers do normalize paths, so the authorization check is inconsistent with enforcement preparation.

Fix: normalize/canonicalize grants at creation and persistence boundaries, reject unresolved traversal/symlink ambiguity, and compare resolved identities immediately before launch. Test `..`, symlink swaps, missing descendants, case-insensitive filesystems, and Windows path prefixes.

### P3 — Test coverage proves mechanics, not production ABI and deadline behavior

The IPC tests use an in-process `ScalePlugin` and validate basic mapping, permissions, and sequence mechanics (`external_plugin_ipc/tests.rs:53-188`). Isolation tests exercise fallback/supervision, but no test asserts allocation/syscall freedom, latency-aligned fallback, real deadline distributions, native ABI fixtures, sample-accurate automation, live state capture, or non-stereo scanner-to-worker negotiation. The scanner tests deliberately scan stub files, which cannot validate metadata. Available user documentation is also sparse: the root README discusses producing AU/CLAP/VST3 plugins, while the only focused changelog entry covers sandbox parsing rather than hosting limitations (`README.md:176`; `crates/sotf-plugins/CHANGELOG.md:27`).

Fix: add small deterministic CLAP/VST3/AU fixture plugins and an end-to-end matrix for load/process/automation/state/latency/channel negotiation. Add real-time guard instrumentation and document feature gates, isolation latency, fallback semantics, unsupported event types, and sandbox guarantees.

## Backend-specific assessment

- **CLAP:** lifecycle cleanup and extension/null checks are generally defensive. Highest gaps are ignored host requests, missing transport/events, block-zero automation, and incomplete handling of sleep/tail/restart semantics.
- **VST3:** COM ownership and state/component-controller bridging are substantial, but bus/event/transport coverage is narrow, automation is offset zero, and controller/process thread-affinity assumptions need explicit enforcement.
- **Audio Unit:** allocation is correctly channel-sized; the initially suspected multichannel out-of-bounds issue is not present. The backend still uses the same oversized planes/scalar transpose, immediate parameter writes, a single global bus model, and limited host musical context.
- **Isolation/sandbox:** file ownership, symlink rejection, header/version checks, panic containment, launch-policy validation, and quarantine counters are good foundations. They do not compensate for the synchronous polling/remapping architecture or the loss of state/automation equivalence.

## Reviewed surface

Read in full or by complete implementation section: `external_plugin.rs`; every module under `external_plugin/`, including CLAP/VST3/AU backends and tests; host/worker/process binaries; every module under `external_plugin_ipc/`, `external_plugin_isolated/`, and `external_plugin_sandbox/` plus tests; the external sandbox QA binary; factory parse/validate/create/catalog paths; descriptor/state/preset bridges; Cargo feature definitions; relevant README/changelog material; and external-plugin isolation tests. Feature-disabled and platform-gated branches were inspected as code even where this macOS default build does not compile them.

## Verification

Focused verification was run after the read-only audit:

- `cargo test -p sotf-host external_plugin`: the unit subset passed 90 tests, but the command also selected the integration binary and its first run failed 4 of 5 worker-process tests (latency metadata timeout / worker exit not observed). An immediate direct rerun of that integration target passed all 5 tests, so this is a reproducible-warning candidate for worker-test timing rather than a deterministic assertion failure.
- `cargo test -p sotf-host --test external_plugin_isolation`: **5 passed** on direct rerun.
- `cargo test -p sotf-plugins factory`: **34 passed**, 245 filtered out.
- `cargo check -p sotf-host`: **passed**.
- Review formatting: `git diff --check --no-index /dev/null reviews/20260812-plugin-external.md` passed; the artifact is 93 lines / 1,685 words before this final result update.

Native-format feature builds were not forced because the workspace default enables none of the optional format backends and this review made no source changes. Their compiled implementations were nevertheless read in full.
