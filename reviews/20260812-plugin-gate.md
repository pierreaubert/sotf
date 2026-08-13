# Gate plugin code review — 2026-08-12

## Findings

### P1 — `range_db = 0` disables gating instead of providing “unlimited” attenuation

The public architecture documentation and `GatePluginParams` field comment define zero as unlimited attenuation, but `calculate_gate_attenuation` always executes `atten.min(self.range_db.max(0.0))` (`crates/sotf-plugins/crates/sotf-plugin-gate/src/lib/gate_plugin.rs:230-259`). With zero, every target attenuation becomes zero and the gate is a passthrough. Zero is a valid advertised parameter value (`src/params.rs:105-106`), so this is reachable from presets and automation.

Choose one contract. For the documented contract, apply the cap only when `range_db > 0`; otherwise leave attenuation uncapped (with a safe numerical ceiling). Add exact steady-state gain tests for range 0, 20, 80, and 120 dB. The existing monotonic property test encodes the current contradictory behavior and must be corrected.

### P1 — Factory/preset construction bypasses nearly all parameter validation

`from_params` passes threshold, ratio, attack, hold, and release directly into `new`; it only partially clamps mix, nonnegative fields, and lookahead (`gate_plugin.rs:65-114,166-208`). NaN survives `clamp`/`max` in several cases, unknown enum strings silently become defaults, and invalid zero/negative timing reaches divisions in `update_coefficients` (`gate_plugin.rs:261-264`). Runtime bridge writes are bounded, but factory JSON takes this separate unchecked path.

Make construction fallible and validate every field with the same `ParamSpec` authority used by runtime parameters. Reject non-finite numbers and unknown choices rather than silently changing them. Test the factory with NaN/infinity, zero/negative timing, out-of-range values, and invalid mode/order strings, asserting errors and finite processing.

### P1 — Runtime lookahead changes alter latency without a graph rebuild and switch delay discontinuously

`lookahead_ms` is not marked structural, and the setter immediately calls `set_delay_ms` (`src/params.rs:128-139`; `gate_plugin.rs:346-372`). Yet reported plugin latency is calculated from that live value (`gate_plugin.rs:605-610`) and compile metadata embeds it (`gate_plugin.rs:318-325`). A host plan compiled at one delay can therefore retain wrong compensation after automation. The delay tap also moves without a crossfade, producing a time discontinuity.

Treat lookahead as a graph/latency-affecting structural parameter and rebuild compensation, or implement a host notification plus crossfaded delay transition. Add chain tests around live 0↔20 ms changes that verify impulse alignment, no missing/duplicated samples, and refreshed total latency.

### P1 — External-sidechain writes can change the graph channel contract in place

The schema correctly marks `sidechain_external` structural, but `apply_values` still assigns it immediately without a side effect (`src/params.rs:85-96`; `gate_plugin.rs:346-374`). `input_channels()` consequently changes from C to 2C (`gate_plugin.rs:330-336`). The process guard turns an unrecompiled graph into an error, but that is an audio dropout rather than safe structural handling.

Reject structural mutation on a live instance or route it exclusively through plugin replacement/recompilation. Test factory/host graph transitions in both directions and ensure processing never observes a channel count different from the compiled buffers.

### P1 — Diagnostic vectors never update, and the scratch holding input levels is overwritten with attenuation

The DSP computes input dB into `monitoring_levels` (`gate_plugin.rs:475-477,532-534`). At cache publication it overwrites that same vector with envelope attenuation (`gate_plugin.rs:576-595`) and passes it to `GateData::update`. That method never writes `input_levels_db`. Worse, `RealTimeCache::new` clones `GateData`; its derived clone shares both nested `Arc<Vec>` values between the two outer cache slots, so `Arc::get_mut(&mut attenuation_db)` is never unique and every attenuation copy is skipped (`src/lib/gate_data.rs:14-31`; `crates/sotf-plugins/crates/sotf-host/src/analyzer.rs:29-38`). Input remains -120 dB and attenuation remains its initial zero forever; only scalar `is_open` changes.

Keep distinct preallocated input-level and attenuation scratch arrays and update both fields in one cache publication. Add linked/unlinked tests that hold a `GateData` snapshot and assert known input level, attenuation, and state after loud and quiet blocks.

### P2 — Reset allocates and rebuilds filter topology

`reset` calls `rebuild_sidechain_hpf` (`gate_plugin.rs:412-423`), which designs filters, allocates a section vector, clones it per channel, and replaces nested vectors (`gate_plugin.rs:266-280`). Hosts commonly reset processing state on realtime lifecycle transitions; this implementation is avoidably allocation-heavy and may free old storage on that thread.

Reset each existing biquad state in place. Reserve filter redesign/allocation for initialization or an off-thread structural update. Add allocation-guarded reset tests with HPF enabled at both orders and high channel counts.

### P2 — HPF/order and detection-mode automation rebuilds or resets DSP state abruptly

Changing HPF frequency/order rebuilds all sidechain filters, and changing peak/RMS calls `LevelDetector::set_mode` on every detector (`gate_plugin.rs:346-370`). Besides allocation risk for HPF changes, both operations discard detector/filter history without transition, so the gate decision can jump. These setup parameters are not marked structural.

Either classify them as structural or prepare state off-thread and crossfade detector control signals. At minimum, document discontinuity semantics and test steady tone/noise automation for clicks, false opens/closes, and zero callback allocation.

### P2 — Channel-link changes reuse incompatible per-channel state

Linked processing updates only envelope/state/counter index 0, while unlinked processing uses every index (`gate_plugin.rs:461-574`). A live `link_channels` toggle performs no state migration (`gate_plugin.rs:346-374`). Leaving linked mode exposes stale envelopes/counters for channels 1..N; entering it makes channel 0 alone authoritative. This can cause channel-dependent jumps and stereo-image movement.

On mode changes, initialize all channel states from a defined linked aggregate and vice versa, with smoothing. Add transition tests from asymmetrical stereo/multichannel signals at several points in attack, hold, and release.

### P2 — Buffer arithmetic is unchecked and zero-channel construction can panic in diagnostics

The buffer guard computes `num_frames * stride` without checked arithmetic and accepts oversized buffers (`gate_plugin.rs:440-459`). After ten calls, linked mode indexes `envelope[0]` even for a zero-channel instance (`gate_plugin.rs:576-587`). Later audio-length multiplication is also unchecked (`gate_plugin.rs:601-602`).

Reject zero channels at construction, use checked multiplication, and define whether exact buffer length is required. Add zero-channel and overflow-shaped context tests that return errors rather than panic.

### P3 — Diagnostic publication rate depends on callback size

The cache publishes every ten `process_in_place` calls (`gate_plugin.rs:576-579`). At 32-frame callbacks this is roughly 150 Hz; at 4096 frames it is about 1.2 Hz, so UI responsiveness depends strongly on host partitioning.

Accumulate processed samples and publish at a sample-rate-derived cadence (for example 30–60 Hz). Test equivalent streams under multiple block partitions.

## Algorithm, allocation, and performance assessment

The gate has a solid basic design: sample-based attack/hold/release, corrected opening/closing coefficient selection, hysteresis in linear space, optional RMS detection, sidechain filtering, channel linking, lookahead, wet/dry smoothing, FTZ/DAZ, and bounded attenuation. The hot loop performs no explicit heap allocation, locking, logging, or I/O. It returns the requested frame count and validates the common short-buffer sidechain failure.

Peak mode still performs per-sample/per-channel `fast_log10` for monitoring and closed-state transfer calculation plus `fast_pow10` for gain; linked mode avoids some duplication. A linear-domain control envelope or less frequent monitoring conversion could reduce cost, and gain application is a SIMD candidate, but correctness and transition semantics should be fixed first. `RealTimeCache` itself uses a two-Arc spare scheme and skips contended updates rather than allocating. Nested `Arc<Vec>` fields defeat its double-buffer ownership model and should be plain pre-sized vectors inside each cache slot.

## Scope reviewed

Read every plugin-owned file without omission: `AGENTS.md`, `README.md`, `USAGE.md`, `UI.md`, `CHANGELOG.md`, `Cargo.toml`, all six source modules, all four unit/integration/property/dynamics test files, and `bin/qa_gate.rs`. Also checked facade exports, factory/catalog/schema registration, parametric/in-place adapters, external-sidechain buffer compaction, host smoothing/level detector/lookahead/cache behavior, allocation coverage, and relevant filter design contracts. No production code was changed.

## Verification performed

- `cargo test -p sotf-plugin-gate`: 50 tests passed across five suites.
- TokenSave context, file inventory, test-risk, and host-contract queries preceded direct reads. TokenSave identified `calculate_gate_attenuation` as the highest-risk untested helper and only 11% symbol-level transitive test coverage in the crate graph.

## Suggested verification after fixes

- Run the crate suite, realtime allocation suite, and QA binary with HPF/RMS/lookahead/external-sidechain variants enabled.
- Add block-partition equivalence tests for attack, hold, release, threshold/mix ramps, state publication, and link transitions.
- Benchmark peak/RMS, linked/unlinked, internal/external sidechain, and 0/20 ms lookahead from mono through 12 channels.
- Verify host latency compensation and graph rebuilding with impulses through neighboring plugins.

## Remediation status

- P1 range semantics: fixed; zero is unlimited with a 240 dB finite safety ceiling. Unit, steady-state, and property regressions cover 0/20/80/120 dB in the correct attenuation order.
- P1 factory validation: fixed; `try_from_params` derives every numeric bound/default from `ParamSpec`, rejects non-finite values, invalid choices, and zero/overflowing channel layouts, while serde rejects unknown fields. `gate_factory_rejects_invalid_or_unknown_preset_state` exercises the public factory path.
- P1 live latency/sidechain topology mutation: fixed conservatively by marking all six topology/latency parameters structural. Exact no-ops succeed; actual changes after initialization reject transactionally, including mixed batches.
- P2 HPF/order metadata mismatch: fixed; `sidechain_hpf_order` is now marked structural to match the live setter's graph-rebuild requirement.
- P1 diagnostics: fixed; independent preallocated input/attenuation scratch and independent cache payload slots now publish both vectors, with linked attenuation mirrored across every output channel. Regression coverage now checks linked and unlinked stereo snapshots.
- P2 reset allocation: fixed; existing HPF biquads, detectors, lookahead buffers, smoothers, diagnostic counters, and scratch are reset in place. The 12-channel fourth-order-HPF allocation guard proves reset performs zero allocations.
- P2 topology transitions: HPF frequency/order, detector mode, link mode, external sidechain, and lookahead reject actual post-initialize changes. Pre-initialize link migration remains defined for construction/control-plane use.
- P2 buffer/zero-channel safety: fixed with fallible construction, checked channel/frame multiplication, exact buffer sizing, initialization/sample-rate enforcement, zero-frame coverage, and oversized/overflow regression tests.
- P3 publication cadence: fixed; diagnostics publish at a 30 Hz sample-count cadence rather than callback count. Partition-equivalence and held-snapshot tests cover cadence and cache ownership.
- Additional exhaustive-audit fixes: live setters are direct and allocation-free; lowering Hold clamps active counters; non-finite programme/sidechain input is converted to silence before DSP state; external sidechain samples remain unchanged; plugin version and compile metadata are tested against crate/runtime state.
- Retained lookahead hardening: replacement instances now have exact impulse
  coverage at 0/5/20 ms, while rejected live changes are proven to preserve
  delay history and latency bit-exactly. The error contract explicitly leaves
  aligned old/new graph crossfading to the host; this base has no plugin latency
  notification or aligned replacement API.
- Retained settled-kernel optimization: monitoring dB conversion now runs only
  for the final sample retained by each callback, keeping open-gate decisions in
  linear space. Criterion improved by median 71%/81%/64% at 256/512/1024 stereo
  frames and repeated within noise.
- SIMD gain application remains deferred pending separate benchmark evidence.
