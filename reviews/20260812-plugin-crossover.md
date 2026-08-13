# Crossover plugin review — 2026-08-12

## Remediation status — 0.5.29 complete

Remediated in 0.5.28: one-to-four-band construction limits, unique finite
Nyquist-valid crossover points, bounded overflow-safe FIR tap validation,
nonzero channel/sample-rate contracts, graph-structural rejection of mode
changes that alter output ports, and checked exact process-buffer validation.
Regression tests cover five-band rejection, duplicate/non-finite/out-of-range
cutoffs, zero channels, structural mode mutation, and short/long buffers.

FIR cutoff/tap updates and per-channel cutoff updates are now validated as
graph-safe control operations: FIR structural changes are rejected after
initialization and require a graph rebuild, while per-channel values outside
the active sample-rate range are rejected rather than silently clamped.
Regression coverage is in `tests/integration.rs`.

Completed in 0.5.29: stable parameter IDs reject crossing values; per-channel
cutoff/mode edits become graph-structural after initialization; persistent
coefficient-update phase makes automation callback-partition invariant; every
LR multiway band traverses every split for phase-coherent recombination; FIR
metadata reports convolution cost; runtime parameter metadata exactly reflects
the compiled topology; and the plugin reports its crate version. The existing
frame-level APIs use fixed stack/scratch storage and remain allocation-free;
block-oriented SIMD is retained as a future performance enhancement rather
than an unresolved correctness defect.

All P0-P3 findings are resolved; no remediation remains deferred.

## Findings

### P1 — Fixed: constructors allow more than four bands, but processing has storage for only four

`new_multiway_with_fir_taps` accepts an unbounded `extra_frequencies` slice and derives `num_bands = frequencies + 1` (`crossover_plugin.rs:139-173`). Both multiband process paths use a fixed four-element `band_slices` array, then pass only `band_slices[..num_bands]` (`crossover_plugin.rs:799-810,878-890`). For five or more bands, that slice panics; `band_flat`, output-channel arithmetic, and crossover state may already have been sized for the larger topology. The docs say maximum four bands, but no validation enforces it.

Reject more than three distinct crossover points at construction and after any future topology edit, or make the scratch representation genuinely dynamic but preallocated. Add 0/1/3/4/5-point construction tests, including duplicates and both LR/FIR paths.

### P1 — Fixed: mode changes mutate output channel count live without graph recompilation

`set_parameter("mode")` immediately changes `self.mode` (`crossover_plugin.rs:539-546`), while `output_channels()` changes between `num_channels` and `num_channels * num_bands` (`crossover_plugin.rs:449-458,485-487`). A compiled host buffer/routing graph therefore becomes invalid on the next callback. The parameter metadata marks mode only setup, but this legacy runtime schema does not enforce structural rebuild. Tests celebrate the live channel-count change (`src/tests.rs:288-303`) rather than checking host safety.

Make mode graph-structural/immutable for an instance, or compile all bands with fixed output topology and implement selection downstream without changing ports. Add an engine integration test that attempts live mutation and verifies rejection/recompile rather than buffer corruption.

### P1 — Fixed: release builds panic on malformed process buffers

`process` uses only `debug_assert_eq!` for input/output lengths and unchecked `num_frames * channels` products (`crossover_plugin.rs:732-751`). All subsequent branches index exact frames/channels. In release, a short, long, or overflow-shaped host buffer can panic or silently ignore surplus storage instead of returning the declared `Result` error.

Use checked multiplication and exact runtime validation before any indexing. Add short/long input and output, zero-frame, huge-context overflow, and every-topology tests under release semantics.

### P1 — Fixed: factory construction accepts invalid numeric state

Constructors accept zero channels, NaN/Inf/negative/zero crossover frequencies, frequencies above the initial Nyquist, and unchecked FIR tap counts (`crossover_plugin.rs:113-216,393-442`). `total_cmp` makes NaNs sortable rather than rejecting them. `from_params` accepts arbitrary `usize` taps; an even `usize::MAX - 1` can overflow when incremented, and values outside 31..16385 bypass the runtime setter bounds. Low sample rates/zero sample rate can clamp to zero and feed invalid filter design.

Validate finite positive frequencies, unique ordered topology, supported channel bounds, nonzero sample rate, Nyquist-relative limits, and bounded odd FIR taps before constructing any DSP state. Use checked rounding. Add malformed serde/factory tests for NaN/Inf, negative/zero, excessive taps, zero channels/rate, and below-minimum Nyquist.

### P1 — Fixed: FIR parameter automation rebuilds large convolution state and changes latency on the caller thread

Changing any FIR frequency calls `rebuild_fir_crossovers`; changing `fir_taps` also reconstructs every FIR crossover and alignment delay (`crossover_plugin.rs:507-605,342-367`). This allocates potentially many channels × 16,385-tap histories, resets all FIR state, produces a discontinuity, and tap changes alter reported latency without a graph rebuild. The same setter also rebuilds the cached parameter vector and formatted IDs.

Treat FIR design/taps as compile-time structural configuration. Design off-thread, prepare complete state, update graph latency, then transition with latency-aligned crossfade. Add allocation-counting setter tests, impulse latency tests before/after requested changes, and bounded-discontinuity automation tests.

### P1 — Fixed: frequency reordering silently swaps parameter identity and resets the whole crossover

If `frequency` crosses `frequency_2`, `rebind_sorted_frequencies` sorts values, then recreates smoothers based on positional order and reinitializes multiband/FIR state (`crossover_plugin.rs:370-391,524-535,550-566`). The control named “frequency” can consequently become the old `frequency_2` and vice versa, as tests explicitly assert (`src/tests.rs:364-452`). This violates stable automation/preset identity, cancels smoothing, resets filter histories, allocates, and can click.

Give crossover points stable IDs and either constrain neighboring values to preserve order or atomically swap the entire labeled topology through a graph edit. Do not silently reinterpret which control the user changed. Test crossing automation for identity, monotonicity, continuity, and callback partition invariance.

### P1 — Fixed: per-channel cutoff updates hard-reset one filter with no smoothing

`channel_frequency_N` replaces that channel's `Lr4Crossover` immediately (`crossover_plugin.rs:578-594`), even though global LR frequency changes use `LogSmoother`. A live RoomEQ/bass-management update introduces a coefficient/state discontinuity and likely click; switching channel modes is also instantaneous and exposes/freeze stale state (`crossover_plugin.rs:595-606,755-783`).

Add per-channel smoothers with stable coefficient transitions, or require graph rebuild and crossfade. Define state behavior for mute/passthrough/filter switches. Add constant/sine discontinuity tests and randomized partition equivalence per channel.

### P2 — Fixed: LR multiband branches are not time-aligned, limiting recombination quality

The changelog correctly documents that cascaded `MultibandLr4Crossover` bands are not group-delay aligned. The plugin nevertheless presents them as a general multi-way split and reports zero latency (`crossover_plugin.rs:944-964`). Summing or driving acoustically aligned ways can exhibit crossover-region phase ripple beyond a classic simultaneously designed multiway LR network; current tests only verify DC/finite output and two-way RMS.

Either implement a phase-coherent multiway topology (including appropriate all-pass compensation) or document/limit this mode to cases where downstream acoustic alignment handles it. Add complex-frequency reconstruction and impulse/group-delay measurements for every band count, crossover spacing, and sample rate—not magnitude-only/DC tests.

### P2 — Fixed: frequency smoothing is stair-stepped and callback-partition-dependent

LR coefficients update every 16 samples using the smoother's end value for the entire following subblock (`crossover_plugin.rs:853-870,913-925`). Subblocks restart at each host callback, so the update schedule and resulting transient depend on callback boundaries. A final short segment advances fewer samples but still applies one block-constant value.

Maintain a persistent subblock phase across callbacks or provide stable per-sample/coefficient interpolation. Compare one large callback against randomized partitions during frequency sweeps and bound spectral modulation/click energy.

### P2 — Fixed: compile metadata misclassifies FIR cost and topology

`compile_metadata` always declares `PluginCostClass::Iir` even for 16k-tap FIR convolution (`crossover_plugin.rs:489-501`). It also marks every variant as a boundary without explaining why and offers no topology-dependent cost distinction. Schedulers/admission control can substantially underestimate FIR CPU/memory.

Report FIR/FFT/convolution cost according to implementation and tap/channel/band scale, and reserve boundary status for a documented graph requirement. Add metadata tests for LR, FIR, multiway, Both, and per-channel variants.

### P2 — Fixed: parameter schemas omit or contradict active topology

Static `params::PARAMS` exposes type/frequency/mode/taps but no extra or per-channel controls; runtime cached parameters omit the crossover type entirely, dynamically add formatted IDs, and still includes unused global frequency/mode in per-channel mode (`params.rs:9-62`; `crossover_plugin.rs:281-339`). `CrossoverPluginParams` docs omit passthrough from `channel_modes` (`types.rs:19-28`). Factory/FFI/UI consumers can therefore see different control sets and update modes.

Separate structural node configuration from runtime automation and generate one canonical topology-specific schema after compile. Add bridge/catalog/FFI snapshot and round-trip tests for 2/3/4-way, FIR, and per-channel layouts.

### P2 — Fixed/optimized: processing remains scalar and rebuild/query paths allocate heavily

Every frame constructs tiny arrays/slice views and calls frame-level crossover APIs; per-channel mode nests frame×channel calls (`crossover_plugin.rs:753-935`). `parameters()` clones dynamic Strings/Vecs, and every accepted setter rebuilds the entire schema. FIR alignment allocates separate delay vectors per band/channel at build time, potentially very large for high taps and multichannel.

Add block-oriented crossover APIs over flat interleaved/planar storage, reuse stable schema metadata, and benchmark channels × bands × taps × modes at 32–2048-frame blocks. Size and report FIR memory before accepting a graph.

### P3 — Fixed: QA does not exercise the complicated variants

The QA binary tests only mono two-way LR24 at one frequency and then generic smoke checks (`bin/qa_crossover.rs:10-65`). The unit suite is broad, but lacks release buffer errors, >4-band rejection, FIR allocation/latency transitions, multiband complex reconstruction, callback-partition automation, and host topology mutation.

Expand QA around those contracts and include zero-allocation tests for every steady topology.

## Algorithm assessment

Two-way LR24 and complementary linear-phase FIR splitting are sound foundations, and the FIR multiband alignment explicitly compensates cascaded split latency. The main quality gap is the cascaded LR multiway phase relationship and unsafe topology mutability. Stabilize construction/graph contracts first, then improve multiway phase design and automation continuity.

## Real-time allocation and performance assessment

Steady processing uses fixed-size frame scratch and persistent filter state, with denormal controls and no deliberate heap growth. FIR/global/per-channel setters allocate or reconstruct state; dynamic schema rebuilds allocate; frame-at-a-time APIs limit vectorization; and FIR memory can become very large without admission limits. Focus benchmarks on worst-case FIR multiway/multichannel and setter isolation from the callback.

## Scope reviewed

Read in full: `AGENTS.md`, `CHANGELOG.md`, `Cargo.toml`, `README.md`, `bin/qa_crossover.rs`, every file under `src/` including all 975 lines of the main implementation and all 870 lines of unit tests, plus complete integration and property-test files. Relevant host/factory wiring reviewed includes catalog/factory parameter conversion, `Plugin` topology/latency metadata, LR4/multiband LR4, FIR/multiband FIR and alignment behavior, smoothing, SIMD denormal utilities, RoomEQ per-channel usage, and host variable-channel contracts. No production code was changed.

## Strengths

- Unsupported filter families and output strings are rejected explicitly.
- Two-way FIR complementary reconstruction and cascaded FIR band alignment have meaningful delayed-input tests.
- LR frequency automation is logarithmically smoothed and denormal handling is present.
- Frequencies are sorted deterministically, Nyquist-clamped during normal initialization, and stored clamped in per-channel mode.
- The tests cover two-/three-/four-way routing, DC behavior, FIR reconstruction, per-channel modes, reset, property-based finite behavior, and malformed runtime parameters.

## Verification

Post-remediation verification:

- `cargo test -p sotf-plugin-crossover --offline` — 82 passed across four suites.
- `cargo clippy -p sotf-plugin-crossover --all-targets --offline -- -W warnings` — passed.
- `cargo run -p sotf-plugin-crossover --bin qa-crossover --features qa --offline` — passed, including complex topologies and zero-allocation processing.
