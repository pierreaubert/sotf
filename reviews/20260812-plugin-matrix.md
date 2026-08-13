# Matrix plugin code review

## Metadata follow-up (0.5.93, 2026-08-13)

Preset selection is now advertised as structural, matching its routing-table
rebuild behavior. `preset_is_advertised_as_structural` guards the host-visible
contract.

## Remediation status

Verified and remediated on branch `fix/20260812-plugin-review-findings`. Every P1–P3 finding below is now covered by a deterministic regression test; no Matrix P0 finding was reported.

| Finding | Status | Regression evidence |
|---|---|---|
| P1 global gain no-op | Fixed | `global_gain_scales_audio_and_parameter_metadata_is_current` |
| P1 dB/linear mismatch and stale values | Fixed | Crosspoints are explicitly linear `-1..1`; `parameter_metadata_reports_live_values` verifies live schema values |
| P1 malformed IDs/index aliasing | Fixed | `malformed_dynamic_parameter_ids_are_rejected_without_panicking`, `dynamic_parameter_indices_cannot_alias_other_crosspoints` |
| P1 sparse constructor validation | Fixed | `sparse_constructor_validates_matrix_and_maps` plus factory tests |
| P1 cold allocation/buffer validation | Fixed | Block scratch removed; allocator-backed `test_matrix_cold_irregular_process_and_realtime_edits_zero_alloc`; exact short/long/rate and zero-initialization-rate tests |
| P1 realtime metadata rebuild allocation | Fixed | Immutable descriptors plus live values on query; allocator test covers gain/phase/mute/solo/dim edits |
| P1 bridge configuration loss | Fixed | Bridge configuration fixtures preserve nonsquare topology and state |
| P1 channel-state width | Fixed | `channel_states_require_exact_output_width` verifies empty/short/long rejection and atomic preservation; builder is fallible |
| P2 stale zero routes | Fixed | `faded_zero_connections_are_pruned` |
| P2 instantaneous phase | Fixed | Signed coefficient smoothing; `phase_toggle_is_smoothed_and_partition_invariant` |
| P2 misleading/non-atomic presets | Fixed | 2→2 pass-through, normalized SMPTE 5.1→2, defined WAVE→AAC remap; headroom, identifiable-channel, and rollback tests |
| P2 solo/reset contracts | Fixed | `solo_N` register/set/get and multi-solo tests; `reset_and_reinitialize_snap_transitions_to_configured_targets` |
| P2 channel-gain hot pass | Fixed | Block scratch removed and all-unity state bypassed; cold/irregular allocation test |
| P3 docs/version/tests drift | Fixed | Docs match runtime presets/API; crate and `PluginInfo` use 0.5.92; changelog and focused regressions updated |

The review's suggested deadline-distribution benchmark matrix remains useful future performance characterization, but it is not a correctness defect and is not used as a substitute for any remediation above.

Date: 2026-08-12

Scope: `sotf-plugin-matrix`

Focus: correctness, routing/mixing quality, realtime allocation, and performance

Plugin file references below are relative to
`crates/sotf-plugins/crates/sotf-plugin-matrix/` unless a repository-relative
path is shown.

## Findings

### P1 — [Fixed] The generated global `gain` control has no effect on audio

The sole centralized `ParamSpec` exposes `gain` as a 0–1 “Matrix routing
coefficient” and the generated layout renders it as the plugin's large primary
knob (`src/params.rs:21-43`). `set_parameter` accepts the value through
`param_bridge` and stores it in `self.gain` (`src/lib/matrix_plugin.rs:508-515`,
`230-240`). The process equation never reads `self.gain`: output uses only the
per-connection smoother, phase sign, and channel-state gain
(`src/lib/matrix_plugin.rs:715-728`). The existing test verifies only that the
field became 0.75, not that output changed (`src/lib/matrix_plugin.rs:1287-1294`).
Thus the generic/generated UI's only visible control is an audible no-op.

Fix: decide whether `gain` is a master output gain or merely a scale definition
for the custom matrix UI. If it is a master, give it an unambiguous unit/range,
smooth it per sample, and multiply every output by it. If it is UI metadata,
remove it from runtime parameters and represent the grid scale separately.
Test 0, intermediate, and unity values against output amplitude, including
automation and varied block partitions.

### P1 — [Fixed] Crosspoint parameters mix dB metadata with linear DSP values and report false current values

Every dynamic `gain_{in}_{out}` parameter is published with range -144..+24 and
current value 0.0 (`src/lib/matrix_plugin.rs:190-205`), matching the documentation's
dB range (`USAGE.md:13-18`). Yet `set_parameter` passes the number directly to
`set_gain`, and the process loop multiplies samples by it as a linear coefficient
(`src/lib/matrix_plugin.rs:530-546`, `325-333`, `723-728`). A host automating
`gain_0_0=6` therefore applies ×6 (+15.56 dB), not +6 dB (×1.995). Conversely,
-6 is phase-inverted ×6, not -6 dB. The UI specification separately says the
array stores linear gains while displaying dB (`UI.md:47-63`), so the host-facing
parameter contract has no single interpretation.

The metadata's current value is always 0.0 even for an identity coefficient of
1.0 or any edited matrix. Mute/dim parameters are likewise rebuilt with `false`,
and `channel_states` with `"[]"`, regardless of actual state
(`src/lib/matrix_plugin.rs:193-222`). This matters directly to FFI, which falls
back to `Plugin::parameters()` for Matrix (`crates/sotf-plugins/crates/plugins-ffi/src/parameter_map.rs:437-440`).

Fix: choose one canonical storage/API unit. Prefer linear DSP storage plus a
host parameter explicitly converted to/from dB (with a defined -infinity
sentinel), or publish a bounded linear coefficient. Build parameter current
values from `matrix`, `channel_states`, and `preset`; keep setter/getter/schema,
serialization, FFI, and custom UI consistent. Add parameter-to-output tests for
-infinity, -6, 0, and +6 dB and metadata-current-value tests after every edit.

### P1 — [Fixed] Malformed dynamic IDs can panic, and invalid input indices alias another crosspoint

The setter parses `gain_*` by splitting into a `Vec` and unconditionally indexing
`parts[1]` and `parts[2]`; phase inversion similarly indexes two components
without checking length (`src/lib/matrix_plugin.rs:530-556`). The getter repeats
the pattern (`src/lib/matrix_plugin.rs:623-635`). IDs such as `gain_0` or
`phase_invert_0` therefore panic instead of returning the trait's error/`None`.
These IDs can arrive through generic automation, FFI, or corrupted state.

Even well-formed numeric IDs are not validated per dimension. `set_gain`
computes `output*num_inputs + input` and checks only the flat index
(`src/lib/matrix_plugin.rs:325-334`). On a 2×2 matrix, `(input=2, output=0)` has
flat index 2 and silently edits the valid `(input=0, output=1)` cell. Getters and
phase inversion have the same aliasing behavior (`src/lib/matrix_plugin.rs:337-363`).

Fix: parse with exact arity (`split_once`/bounded parser), reject trailing or
missing fields, and validate `input < num_inputs && output < num_outputs` before
forming the index. Return descriptive errors. Fuzz setter/getter IDs and test
each dimension boundary, huge indices, missing/extra components, non-ASCII, and
integer-overflow cases; assert no panic and no unrelated cell mutation.

### P1 — [Fixed] Sparse construction can panic on malformed matrix data exposed by the factory

`with_sparse_mapping` rejects empty maps but never verifies that `matrix.len()`
equals `input_channel_map.len() * output_channel_map.len()`
(`src/lib/matrix_plugin.rs:133-169`). It immediately calls
`update_active_connections`, which indexes `gain_smoothers[idx]` for every
logical crosspoint (`src/lib/matrix_plugin.rs:243-255`). A short matrix therefore
panics inside the supposedly fallible constructor; an oversized matrix creates
state vectors whose tail is inconsistent and unreachable. The primary JSON
factory passes untrusted sparse maps and matrix data directly into this path
(`crates/sotf-plugins/src/factory/create.rs:680-707`), so a malformed preset can
terminate plugin creation rather than return its advertised error.

The constructor also derives physical channel counts as `max + 1` without a
checked addition (`src/lib/matrix_plugin.rs:141-142`), and does not define
whether duplicate physical indices are allowed intentional summing or invalid
maps.

Fix: validate checked dimensions, exact matrix/phase lengths, map index bounds,
and duplicate policy before constructing any smoother or cache. Cap dimensions
to host-supported widths. Add sparse factory tests for short/long matrices,
empty/duplicate/huge maps, checked-overflow values, and valid noncontiguous
maps; require errors rather than panics.

### P1 — [Fixed] `process` allocates on the first or any larger block and does not validate buffers

The plugin starts with an empty `ch_gains_buffer` (`src/lib/matrix_plugin.rs:55-69`).
`process` resizes it to `physical_outputs * num_frames` whenever the host presents
a larger block (`src/lib/matrix_plugin.rs:674-690`). The catalog nevertheless
marks Matrix zero-allocation (`crates/sotf-plugins/src/factory/catalog.rs:1037-1057`).
The allocation regression warms the same fixed block 20 times before measuring
(`crates/sotf-plugins/tests/realtime_allocation_tests/tests.rs:679-698`), so it
cannot detect the initial allocation or a later maximum-block increase. This
can glitch at first playback or after device/host block-size changes.

The method also does no exact input/output length or sample-rate validation
before indexing by `context.num_frames` (`src/lib/matrix_plugin.rs:674-728`). A
short buffer panics; an oversized output is entirely zeroed even when only part
belongs to the declared frame count; a mismatched context rate changes the
automation time constant relative to the stream without error.

Fix: allocate maximum-block scratch in an explicit prepare/build contract, or
eliminate block-sized scratch by retaining only one preallocated per-output
gain vector and advancing it frame-by-frame before the connection loop. Validate
checked expected lengths and initialized sample rate before mutating state or
output. Test cold first call and increasing/irregular blocks under the allocator,
plus short/long buffers, zero frames, multiplication overflow, and rate mismatch.

### P1 — [Fixed] Every realtime crosspoint edit rebuilds and allocates the entire parameter surface

Changing one gain or phase first scans the whole matrix and pushes active
connections (`src/lib/matrix_plugin.rs:243-276`, `325-356`), then
`set_parameter` calls `rebuild_cached_parameters` (`src/lib/matrix_plugin.rs:530-562`).
That rebuild allocates a new vector and formats multiple IDs and names for every
crosspoint, plus channel controls (`src/lib/matrix_plugin.rs:172-228`). A dense
16×16 matrix creates more than 500 dynamic parameter objects/strings for every
single automation event. Adding connections can also grow the two active-cache
vectors because their constructor capacity initially reflects only the identity
diagonal.

Mute/dim/channel-state edits also resize state/smoother vectors and rebuild the
same O(inputs×outputs) metadata (`src/lib/matrix_plugin.rs:564-605`). The
rapid-update test checks functional errors but does not assert allocation or
deadline behavior (`crates/sotf-plugins/tests/all_plugins_parameter_robustness.rs:350-383`).

Fix: establish a prepared control-to-audio transfer. Keep parameter descriptors
immutable for fixed dimensions, update only current-value storage, reserve all
matrix/cache capacities at construction, and pass coefficient targets through
a bounded lock-free/event mechanism or documented non-audio control boundary.
Batch preset/matrix changes into one cache rebuild. Add zero-allocation and
p95/max deadline tests for single-cell drags, dense paste, phase, mute/solo/dim,
and presets at 2×2 through 16×16.

### P1 — [Fixed] The plugin bridge discards the complete Matrix configuration

The main factory parses dimensions, sparse maps, matrix coefficients, and
channel states (`crates/sotf-plugins/src/factory/create.rs:680-741`). The
plugins bridge instead ignores `config_json` and always constructs a square
identity matrix from the incoming channel count
(`crates/sotf-plugins/crates/plugins-bridge/src/factory.rs:184-193`). Any saved
swap, downmix, RoomEQ route, mute, or custom matrix silently becomes identity
when instantiated through that surface. Because construction succeeds, callers
receive no indication that their requested routing was lost.

Fix: share one fallible parameter type and constructor between the primary
factory and bridge. Preserve nonsquare output count and sparse maps, validate
the incoming host channel count, and return errors consistently. Add identical
configuration fixtures through primary factory, bridge, engine converter, FFI,
and state round-trip, comparing channel counts and sample-exact routing.

### P1 — [Fixed] Runtime channel-state JSON can silently mute every real output

`channel_states` accepts an arbitrary-length JSON vector and stores it without
normalizing to `num_outputs` (`src/lib/matrix_plugin.rs:598-604`). Both smoother
builders compute `any_soloed` over the entire vector, then apply that global flag
only to real output indices (`src/lib/matrix_plugin.rs:372-395`, `465-485`). If
an extra out-of-range state is soloed, `any_soloed` is true while no actual
output is soloed, so every real output target becomes zero. The primary factory
does resize state to the output width (`crates/sotf-plugins/src/factory/create.rs:729-738`),
but live parameters and the public `with_channel_states` method do not.

Fix: require exactly `num_outputs` states, or truncate/pad under an explicitly
documented migration rule before computing solo. Validate atomically before
changing current state. Test empty, short, exact, and oversized vectors with
solo flags in valid and invalid positions for full and sparse mappings.

### P2 — [Fixed] Zero-target connections remain permanently in the hot cache after smoothing

An active connection is retained when its target is nonzero or its current
value is still transitioning (`src/lib/matrix_plugin.rs:243-255`). This is
necessary while fading to zero. Once the smoother converges, however, nothing
prunes the entry: the process path intentionally never calls
`update_active_connections` (`src/lib/matrix_plugin.rs:704-714`). A connection
removed by the user therefore continues to advance and multiply/add zero every
sample until some later unrelated matrix mutation happens to rebuild the cache.
Repeated editing can leave an almost-dense hot loop for a logically sparse
matrix.

Fix: track a bounded “fading-to-zero” set and remove an entry when its smoother
reaches target, without scanning the entire matrix per block. Alternatively
compact at a safe block boundary from a prepared cache. Benchmark a 16×16 matrix
after enabling then disabling all cells; its steady cost should return to the
true sparse baseline.

### P2 — [Fixed] Phase inversion is instantaneous while gain and channel-state changes are smoothed

Gain targets and mute/dim/solo transitions use 5 ms smoothers, but phase is
pre-resolved to a ±1 sign and replaced immediately on any phase edit
(`src/lib/matrix_plugin.rs:243-273`, `342-356`). Flipping a nonzero connection
from + to - creates a two-times-signal discontinuity at the block boundary and
can click badly at high instantaneous amplitude. The separate negative-gain
path would naturally ramp through zero, but the explicit phase control bypasses
it.

Fix: crossfade phase changes through zero (or between old/new connection
outputs) over the configured transition interval, with a clear policy for
simultaneous gain edits. Test phase toggles at sine peaks, DC, impulses, and
multiple block boundaries; bound first-difference/click energy and verify
block-partition invariance.

### P2 — [Fixed] Presets are misleading and setter failure is not atomic

`stereo_downmix` requires at least 2×2 and produces two cross-blended stereo
outputs, `L + 0.707R` and `R + 0.707L`, rather than reducing channel count
(`src/lib/matrix_plugin.rs:397-419`). Correlated unity stereo reaches 1.707 per
output (+4.65 dB) with no headroom policy; the integration test explicitly
expects that amplification (`tests/integration.rs:201-225`). The changelog
already acknowledges the naming defect (`CHANGELOG.md:31-36`). `5.1_remap` is
only identity for any dimension, not a documented channel-order remap
(`src/lib/matrix_plugin.rs:450-459`). These names invite acoustically unsafe or
incorrect routing choices.

The preset setter assigns `self.preset` before applying it
(`src/lib/matrix_plugin.rs:517-527`). On a 1×1 matrix, selecting stereo downmix
returns an error but leaves metadata claiming that preset while the old matrix
remains. Out-of-range indices are silently clamped rather than rejected
(`src/lib/matrix_plugin.rs:518-522`, tests at `1329-1338`).

Fix: rename the crossblend with a state migration, implement actual N→2/N→1
downmix presets with specified channel roles and headroom laws, and define the
5.1 source/destination orders. Prepare a candidate matrix first and commit
preset+matrix atomically only on success; reject invalid indices. Test correlated,
anti-correlated, and channel-identifiable signals, peak headroom, and failure
rollback for every supported dimension.

### P2 — [Fixed] Public solo/reset contracts are incomplete or misleading

AGENTS says the plugin exposes per-channel `mute_N`, `solo_N`, and `dim_N`
parameters (`AGENTS.md:32`), and the UI specifies an M/S/D sidebar (`UI.md:65-75`).
The cached parameters and setter implement mute and dim only; there is no
`solo_N` branch (`src/lib/matrix_plugin.rs:190-222`, `564-605`). Solo is available
only by replacing the opaque JSON `channel_states` array. This makes generic
host/automation access inconsistent with the documented control surface.

The plugin does not override `reset`; the integration test merely verifies that
processing eventually continues after the trait default no-op
(`tests/integration.rs:319-331`). Any in-progress gain or channel-state smoother
therefore survives a transport/device reset. `initialize` changes smoother time
constants but preserves their current transition state (`src/lib/matrix_plugin.rs:663-671`).

Fix: register/set/get `solo_N` symmetrically, or document and expose one typed
channel-state API rather than JSON. Define reset and reinitialize semantics;
normally snap current values to declared targets and clear transient control
state without changing the routing configuration. Test solo round trips,
multi-solo priority, reset mid-ramp, and 44.1→96 kHz reinitialization against a
fresh instance.

### P2 — [Fixed] The hot path performs a full block-sized channel-gain pass even for default routing

Before mixing, `process` fills `num_frames * physical_outputs` scratch entries,
advancing a smoother or writing 1.0 for every output/sample
(`src/lib/matrix_plugin.rs:685-702`). It then rereads one entry for every active
connection (`src/lib/matrix_plugin.rs:715-728`). Most matrices have no channel
state smoother at all, so the first pass writes a large table of ones merely to
read them back. Sparse output mapping additionally sizes and strides this table
by the largest physical output index rather than logical output count.

The workspace benchmarks cover identity 2×2, identity-like 2→6, and identity
8×8 at one block size (`crates/sotf-plugins/benches/all-plugins-benchmark/benchmark.rs:380-449`).
They do not cover dense matrices, sparse maps with high physical indices,
fading coefficients, channel-state automation, varied blocks, or worst callback
time.

Fix: advance a preallocated logical-output gain vector once per frame and
consume it immediately in the connection loop; bypass channel-gain work entirely
when all targets/current values are unity. Consider specialized identity/copy,
permutation, mono-sum, and dense matrix kernels selected at control time. Measure
p50/p95/max deadline margin across sparse/dense 2–16 channel matrices, blocks
16–4096, transitions, and sparse physical maps.

### P3 — [Fixed] Documentation, versions, and quality tests have drifted from implementation

Package version is 0.5.89 while the changelog begins with 0.5.90 and
`PluginInfo` reports 1.1.0 (`Cargo.toml:1-5`, `CHANGELOG.md:1-8`,
`src/lib/matrix_plugin.rs:489-492`). AGENTS describes the implementation as
`src/lib.rs`, although the 1,398-line implementation moved to
`src/lib/matrix_plugin.rs` (`AGENTS.md:7-11`). USAGE lists Identity, Swap, and
Mono Mix presets with serialized forms such as `"matrix":"identity"`
(`USAGE.md:29-38`, `83-109`), while runtime choices are custom,
stereo_downmix, ms_encode/decode, and 5.1_remap integer indices
(`src/lib/consts.rs:4-21`, `src/lib/matrix_plugin.rs:517-525`). UI claims M/S is
disabled for more than two channels, but DSP accepts any matrix with at least
2×2 and zeroes the remaining routes.

Tests are numerous and useful for basic routing, but omit cold allocation,
buffer errors, malformed IDs/sparse state, bridge parity, current metadata,
global gain audibility, dB conversion, transition partition invariance, preset
rollback/headroom, active-cache pruning, and deadline distributions. Several
tests reinforce defects: non-finite gains are silently accepted as `Ok` while
ignored (`src/lib/matrix_plugin.rs:1221-1247`), preset indices clamp, and the
misnamed stereo downmix is expected to amplify.

Fix: make crate/changelog/plugin versions and all preset/control docs derive
from one canonical schema. Replace behavior-documenting defect tests with
correct error/contract assertions and add sample-exact offline matrix references
for arbitrary N×P, sparse maps, negative coefficients, automation, and varied
block partitions.

## Algorithm and realtime assessment

The intended steady-state equation is a conventional row-major linear matrix:
for each physical output, sum mapped physical inputs multiplied by a smoothed
crosspoint coefficient and a smoothed logical output-state gain. Initial full
matrices are identity over `min(inputs, outputs)`; sparse maps allow logical
submatrices over noncontiguous physical channels. The frame-outer,
connection-inner loop and pre-resolved physical mapping/sign are good choices
for interleaved audio and were an important improvement over the previous
cache-thrashing traversal.

No algorithmic latency is introduced. Coefficients and channel-state gains use
per-sample one-pole smoothing, so gain automation itself is block-size
independent. The major realtime caveats are cold/block-growth scratch allocation,
large metadata reconstruction on edits, cache growth, and never pruning faded
zero connections. The current zero-allocation result proves only warmed,
fixed-size 2×2 processing.

As a general matrix, clipping protection should not be forced into the plugin:
intentional sums may exceed unity. Presets, however, must state and implement a
normalization/headroom law because users reasonably treat them as safe routing
recipes. The current crossblend/downmix naming is the principal algorithmic
quality problem, followed by ambiguous dB-versus-linear automation.

## Strengths

- The row-major matrix convention is documented and consistently used by the
  core full-matrix processing path and engine settings.
- Gain smoothers advance once per sample per active connection, avoiding the
  block-end-value automation bug found in several other plugins.
- Mute/dim/solo state gains are also smoothed, and multi-solo priority works for
  correctly sized state vectors.
- The frame-outer traversal keeps an interleaved frame hot while visiting active
  connections; physical indices and phase signs are resolved outside the sample
  loop.
- `output.fill(0.0)` gives deterministic accumulation semantics for valid
  buffers, including outputs with no routes.
- Full-matrix construction and `set_matrix` validate exact element count.
- Negative linear coefficients and explicit phase inversion both work, and M/S
  encode/decode matrices have focused round-trip tests.
- Sparse mapping supports noncontiguous physical input/output indices and has
  basic correctness coverage.
- The primary factory preserves nonsquare channel topology, resizes legacy
  square matrices to the current host width, and normalizes factory-provided
  channel-state length.
- Tests cover identity, swap, mono sums/pan laws, off-diagonal surround routes,
  M/S, phase, mute/dim/solo, property-based finite output, 12-channel identity,
  parameter round trips, and warmed realtime allocation.
- Workspace benchmarks include small, upmix, and 8×8 cases, and measured QA
  throughput is comfortably below its deadline on the current machine.

## Exhaustive scope reviewed

Every plugin-owned file was read in full:

- Documentation/configuration: `AGENTS.md`, `CHANGELOG.md`, `Cargo.toml`,
  `README.md`, `UI.md`, `USAGE.md`.
- QA: `bin/qa_matrix.rs`.
- Source: `src/lib.rs`, `src/lib/consts.rs`,
  `src/lib/matrix_plugin.rs`, `src/params.rs`, including every inline test.
- Integration/property tests: `tests/integration.rs`,
  `tests/matrix_mixing_test.rs`, `tests/matrix_mute_test.rs`,
  `tests/property_tests.rs`, `tests/test_basic.rs`.

No plugin-owned benchmark, example, fixture, or additional QA file exists.
Relevant workspace integration reviewed included facade exports; primary factory
and legacy resize; catalog metadata/allocation claims; plugins-bridge
construction; FFI dynamic-parameter fallback; engine settings/default matrix,
config conversion and channel-count propagation; plugin-chain permanent Matrix
usage; RoomEQ matrix configuration paths; factory, parameter robustness,
round-trip, realtime-allocation, high-channel tests; and the all-plugin
allocation/performance benchmarks.

TokenSave was used before source reads to locate the active implementation,
symbols, tests, callers, and integration surfaces. It saved approximately
22,400 context tokens during this review.

## Verification

Executed from the workspace root:

```text
cargo test -p sotf-plugin-matrix
  84 passed; 0 failed (7 suites)

cargo test -p sotf-plugins --lib
  38 passed; 0 failed

cargo test -p sotf-plugins --test realtime_allocation_tests test_matrix
  2 passed; 0 failed; 48 filtered out

cargo check -p sotf-plugin-matrix
  PASS

cargo run -p sotf-plugin-matrix --features qa --bin qa-matrix
  identity passthrough: PASS
  reported latency 0: PASS
  zero allocations: PASS
  5.0 s audio in 0.97 ms; estimated CPU 0.02%: PASS

git diff --check
  PASS
```

The focused suite now exercises global-gain audibility, truthful live metadata,
malformed IDs, sparse constructor limits/duplicates, cold and irregular-block
allocation, exact buffer/sample-rate contracts, channel-state width atomicity,
preset rollback/headroom/remapping, phase transition partition invariance,
active-cache pruning, solo controls, and reset/reinitialization semantics.
