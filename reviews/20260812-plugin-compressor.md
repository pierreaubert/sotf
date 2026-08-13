# Compressor code review — 2026-08-12

## Final closure — 0.5.16

All P1–P3 findings are fixed and covered by regressions:

- The `compressor` factory now constructs a true one-band broadband mode, reports `Compressor`, exposes no band/crossover or unsupported sidechain controls, and matches a direct broadband two-tone reference.
- Lookahead is reported and phase-aligns dry, wet, bypassed, passive, and M/S paths through 20 ms. Lookahead and band count are structural after initialization, so a live write cannot invalidate compiled host latency/topology metadata.
- Analyzer snapshots own their vectors, the canonical link amount is continuous, malformed presets and explicitly serialized no-op sidechain fields are rejected, and oversized blocks are processed in bounded allocation-free chunks.
- Crossover frequency, threshold, ratio, knee, attack/release coefficients, makeup, channel link, mix, and sidechain tilt are smoothed sample-by-sample. Tilt filters remain preallocated and preserve state during automation.
- Cascaded LR4 behavior is documented honestly. Swept 2–5-band tests measure bounded magnitude and finite phase with a deep fitted residual/null rather than claiming phase-perfect reconstruction.
- Focused evidence includes dry M/S matrices, impulse latency at 0/1/5/10/20 ms across partitions, transactional structural writes, live analyzer values, link monotonicity, factory/schema parity, large-block equivalence, automation partition equivalence and sample-jump bounds, invalid factory JSON, swept crossover transfer, and allocation-counted setters/reset/process.

## Remediation status — 2026-08-12

Additional remediation in `sotf-plugin-multiband-compressor` 0.5.14:

- **Fixed:** Catalog/factory identity mismatch. The `compressor` catalog row now explicitly labels
  the implementation as `Compressor (Multiband)`, references the dynamic global/per-band schema,
  and uses custom multiband UI metadata. A factory regression verifies the created runtime reports
  `Multiband Compressor` and exposes `num_bands`.
- **Deferred:** Restoring a true broadband single-band compressor, implementing the advertised
  single-band sidechain controls, or adding a separate adapter remains a broader DSP/schema design;
  the catalog no longer claims those controls or identity for this alias.

Implemented in `sotf-plugin-multiband-compressor` 0.5.13:

- Fixed lookahead latency reporting and aligned dry, bypassed, and passive signal paths with active
  bands. The allocated/clamped range now matches the advertised 0–20 ms contract.
- Fixed M/S dry/wet coordinate handling by decoding the wet sum before L/R mixing.
- Made `num_bands` structural after initialization and reject live changes before any state mutation.
- Fixed analyzer publication by giving each realtime-cache snapshot owned meter/crossover vectors.
- Added regressions for dry M/S passthrough, latency metadata, live analyzer values, transactional
  band-count rejection, and the 20 ms lookahead limit.

Remaining findings are not silently closed: the catalog's true single-band Compressor identity,
sidechain/schema parity, link-control migration, parameter/crossover smoothing, oversized-block
chunking, and crossover reconstruction/documentation require broader factory, host, or DSP design
work. Dynamic lookahead changes also still need an explicit host
latency-recompile notification contract.

Implemented in `sotf-plugin-multiband-compressor` 0.5.15:

- **Fixed:** Factory construction now validates finite, schema-bounded global and per-band
  parameters, sidechain enum values, crossover ordering, and crossover frequencies below the
  actual host Nyquist. Main, bridge, and A/B Compare creation paths use this validation before
  allocating DSP state. Regression tests cover malformed ratio and crossover configurations.

## Findings

### P1 — The catalog's single-band Compressor is instantiated as the default multiband processor

`CompressorPlugin` is a direct alias of `MultibandCompressorPlugin` (`crates/sotf-plugins/src/lib.rs:288`), and the factory deserializes `MultibandCompressorPluginParams` without forcing `num_bands = 1` (`crates/sotf-plugins/src/factory/create.rs:94-99`). The catalog nevertheless presents the single-band `SINGLE_BAND_LAYOUT` (`crates/sotf-plugins/src/factory/catalog.rs:451-477`). The multiband parameter spec clamps `num_bands` to at least 2, so this is not a harmless alias: a nominal Compressor splits the input and detects/compresses each band independently. Its transfer differs from a broadband compressor because each band's detector sees only band-limited level. `info()` also reports “Multiband Compressor,” not the catalog identity.

Fix by adding a real single-band construction mode/adapter that permits exactly one band and reports `Compressor`, or restore a dedicated broadband implementation. Make factory creation explicit rather than relying on a type alias. Add a factory-level test that creates `"compressor"`, verifies its identity/schema, feeds a two-tone signal, and compares it to a broadband reference transfer.

### P1 — Lookahead latency is neither reported nor aligned in the parallel path

The processor delays every active band through `LookaheadBuffer` when `per_band_lookahead_ms > 0`, but it does not override `latency_samples()`; the host trait default is zero. `compile_metadata()` therefore also reports zero latency (`multiband_compressor_plugin.rs:842-850`). The dry path remains undelayed at recombination (`:1167`), so any `mix` between 0 and 1 combines time-shifted wet audio with current dry audio and creates comb filtering. Bypassed/passive bands skip the lookahead branch entirely (`:1019-1030`), so they are also misaligned with active bands.

Fix by reporting the exact active delay, delaying the dry path by the same amount, and applying an equal delay to bypassed/passive bands. Define whether a lookahead change is a structural latency change and notify/recompile the host accordingly. Test impulse timing for wet, dry, bypassed, and 50% mix paths at 0/1/5/10/20 ms, across host block partitions.

### P1 — M/S mode corrupts the dry path, including nominal bypass

In M/S mode the working input is encoded before splitting (`multiband_compressor_plugin.rs:968-978`), but `dry_buffer` retains L/R. Recombination blends L/R dry with M/S wet (`:1167`), then decodes the entire blend as M/S (`:1172-1180`). At settled `mix = 0`, output becomes `L+R, L-R`, rather than the original L/R signal. Intermediate mixes blend incompatible coordinate systems.

Fix by decoding the wet sum to L/R before dry/wet mixing, or encode the dry path and decode both paths consistently. Add exact dry-bypass tests with asymmetric stereo, anti-phase stereo, mono-in-stereo, and automation across mix 0→1. Existing M/S tests only check finiteness and a loose RMS bound, so they cannot catch the matrix error.

### P1 — Runtime band-count changes can allocate and can overrun stale audio storage

The `num_bands` setter grows `band_params`, compressors, lookahead buffers, makeup trackers, meters, cached parameters, crossovers, and tilt filters (`multiband_compressor_plugin.rs:526-569`). These heap operations can occur through the host parameter path. More importantly, it never resizes/rebuilds `band_buffers`, which was sized in `initialize()` from the old count (`:906`). The check before processing is only a `debug_assert`; in release, increasing 2→4 bands and then processing a sufficiently large block indexes beyond the allocation. The existing regression uses 2,048 frames, which happens to fit the old 2×4,096-frame storage and masks the defect.

Treat `num_bands` as a compile/build-time structural parameter. Rebuild off the audio thread and atomically swap a fully initialized processor, or preallocate for the maximum band count and never grow any vector in setters. Add release-mode tests for every band-count transition at 4,096 frames, plus an allocation-count assertion around host automation.

### P1 — Analyzer data never updates because the cache contains nested shared `Arc`s

`MultibandCompressorData` stores each vector as `Arc<Vec<_>>` (`multiband_compressor_data.rs:6-8`). `RealTimeCache::new` clones the initial `T` into two outer snapshots; that clone shares all inner `Arc`s. Consequently every `Arc::get_mut` in `MultibandCompressorData::update` (`:31-45`) fails, and gain reduction, levels, and crossover values remain at their constructor values. Tests validate only lengths or internal plugin vectors, not changed cached values.

Store plain `Vec`s inside the already-`Arc`-protected cache object, or deep-clone the inner storage when creating spare snapshots. Add a test that processes enough audio to trigger a cache update and asserts non-default GR/levels and current crossover values through `get_data()`.

### P1 — The advertised single-band sidechain controls are absent or no-ops

The catalog uses `SINGLE_BAND_LAYOUT`, whose schema exposes sidechain HPF, HPF order, detection mode, program-dependent release, and external sidechain. The runtime deliberately omits these parameters and stores their preset fields without DSP (`multiband_compressor_plugin.rs:44-57, 314-318`). Engine accessors explicitly expect these Compressor keys. Thus the catalog/UI, engine contract, preset surface, and runtime parameter schema disagree.

Either implement the complete detector/sidechain contract or remove the controls from every public schema and migrate presets explicitly. Do not silently accept saved values that have no audible effect. Add a catalog-to-runtime schema parity test and signal tests for every exposed sidechain mode.

### P2 — Link Amount is shadowed by the default Link Channels switch

The detector chooses `link = 1.0` whenever `link_channels` is true (`multiband_compressor_plugin.rs:1058-1062`). Because `link_channels` defaults true, changing the separately exposed continuous `link_amount` has no effect until users also disable the legacy boolean. This is surprising and the current `test_process_link_amount_half` checks only for finite output.

Use one canonical continuous link control; map the legacy boolean to 0/1 only during preset migration. Test unequal L/R levels and assert monotonic, numerically distinct gains at link 0, 0.5, and 1.

### P2 — The declared 20 ms lookahead range is silently capped at 10 ms

Both `PARAMS` and `GLOBAL_PARAMS` advertise 0–20 ms, while construction and setters clamp to 10 ms (`multiband_compressor_plugin.rs:162-164, 277, 648`). Initialization allocates only 10 ms. This breaks parameter round-trip expectations and host latency planning.

Choose one limit, use it in specs, storage, validation, serialization, and latency tests, and reject rather than silently reinterpret invalid saved values.

### P2 — Crossover and dynamics automation are block-dependent or discontinuous

Each crossover smoother advances by an entire block with `next_n(nf)` and then installs one coefficient set for the whole block (`multiband_compressor_plugin.rs:981-984`); output therefore depends on block partitioning during crossover automation. Per-band threshold, ratio, knee, attack, release, makeup, linking, and tilt are unsmoothed. Changing tilt reconstructs detector biquads and discards their state. These operations can click or modulate gain abruptly.

Use bounded coefficient interpolation or a crossfade between old/new crossover banks, and smooth continuous dynamics parameters in suitable domains. Add block-partition equivalence tests and maximum sample-jump tests; the existing “jump < 1.0” bound is too loose for audio quality.

### P2 — The hard 4,096-frame rejection is an avoidable host failure

Unlike the expander's time-domain path, compressor processing returns an error for any block over 4,096 frames (`multiband_compressor_plugin.rs:947-951`). Offline renderers and unusual devices can legitimately use larger blocks. This turns a private allocation policy into a public processing limit.

Chunk oversized calls without allocating, or negotiate a maximum block size during initialization. Verify output/state parity between one large call and multiple 4,096-frame chunks.

### P2 — Construction accepts invalid DSP configuration from factory JSON — Fixed in 0.5.15

Factory deserialization bypasses runtime parameter validation. `with_params` does not reject non-finite/out-of-order crossover frequencies or out-of-range global ratio, attack, release, threshold, knee, mix, and tilt. Invalid filter frequencies can generate unstable/non-finite coefficients; negative or non-finite timing values can corrupt envelope coefficients.

Make `from_params` fallible and validate the same authoritative specs plus strict ascending crossover order below Nyquist. Add malformed-preset tests for NaN/inf, negative times, invalid ratios, duplicate/descending crossovers, and low sample rates.

### P3 — Documentation and tests overstate crossover reconstruction

The usage guide calls the cascaded split phase-coherent and says ratio 1:1 reconstructs the original. The shared LR4 helper explicitly notes that cascaded bands have unequal group delays and are not a phase-perfect split. Current tests accept RMS error bands of ±15% or more and do not measure null depth, phase, or response ripple.

Document the real topology. Add swept-frequency magnitude/phase and null tests for 2–5 bands, and consider an all-pass-compensated or parallel crossover topology if mastering-grade reconstruction is required.

## Realtime and performance assessment

The steady-state process loop is allocation-free after initialization for a fixed structure and blocks up to 4,096 frames. Fast log/pow calls remain per band/channel/sample; profile before SIMD work. More important than micro-optimization are the structural setter allocations, repeated parameter-schema formatting/cloning, hard block limit, and nested-cache bug above. `parameter_schema`, `current_values`, and every setter rebuild allocate; these must not be used for sample-accurate automation on the audio thread.

## Coverage reviewed

Reviewed all crate files: `AGENTS.md`, `README.md`, `CHANGELOG.md`, `UI.md`, `USAGE.md`, `Cargo.toml`, QA binary, every source module, all unit/integration/multiband tests, plus factory aliases/catalog/create paths. Relevant host/shared code reviewed includes `ParametricInPlacePlugin` and its adapter, `RealTimeCache`, `LookaheadBuffer`, `MeasuredMakeup`, `LevelDetector`, `Lr4Crossover`, smoothers, and compiled metadata. No code was changed and no broad workspace build was run.
