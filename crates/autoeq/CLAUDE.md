# autoeq (crate: `autoeq`, version: 0.4.42)

Core automatic equalization stack for speakers, headphones, and RoomEQ workflows.

## High-level workflow

1. `read/`: ingest + normalize measurement data (CSV / API / recording artifacts)
2. `loss/`: compute objective losses (`flat`, `score`, `epa`, multi-driver, multi-sub)
3. `optim/`: run optimizer backends over PEQ params
4. `x2peq.rs` + `response.rs`: turn solution vectors into DSP filters and responses
5. `roomeq/`: multi-channel orchestration, reporting, routing, and export

## Loss types

- `speaker-flat`
- `speaker-flat-asymmetric`
- `speaker-score`
- `headphone-flat`
- `headphone-score`
- `drivers-flat`
- `multi-sub-flat`
- `epa`

## Optimizer backends

Registry path: `src/optim/registry.rs`.

- `autoeq:de` (DE, supports JADE/L-SHADE strategy variants)
- `autoeq:cobyla` (pure-Rust COBYLA)
- `autoeq:isres` (pure-Rust ISRES)
- `autoeq:cmaes` (CMA-ES)
- `autoeq:nsga2`, `autoeq:nsga3` (Pareto MO optimizers)
- `mh:*` backends (e.g. `mh:pso`, `mh:firefly`, etc.)

Notes:
- The old C-FFI NLopt backend is removed. `nlopt:*` names are kept as compatibility aliases and are mapped to pure-Rust backends with warnings.
- Common bare aliases (`de`, `cobyla`, `isres`, `cma-es`, `nsga-ii`, `nsga-iii`) resolve through the registry.

## Key objective controls

- `ObjectiveData` carries single-curve and multi-objective paths.
- `multi_objective` delegates scalarization over per-curve objectives.
- `smoothness_penalty` (TV² curvature regularizer in log-frequency) is optional and supported in PEQ-based branches.
  - Config fields: `tv2_weight`, `schroeder_hz`, `modal_weight_scale`, `exponent`.
  - CLI flags: `--smoothness-weight`, `--smoothness-exponent`, `--smoothness-schroeder-hz`, `--smoothness-modal-scale`.
  - RoomEQ JSON path: `optimizer.smoothness_penalty`.

## RoomEQ highlights

Main tree: `src/roomeq/`.

- Multi-seat strategies include:
  - `minimize_variance`
  - `primary_with_constraints`
  - `average`
  - `modal_basis` (complex-domain SFM modal-basis optimization)
- Multi-measurement strategies include weighted/minimax/variance-penalized paths and spatial robustness mode.
- Bass-management and routed export logic live under `roomeq/optimize`, `roomeq/workflows`, and `roomeq/output`.

## Binary entry points

From `Cargo.toml`:

- `autoeq`
- `benchmark-autoeq-speaker`
- `autoeq-download-speakers`
- `roomeq`
- `roomeq-fuzzer`
- `roomeq-qa-quality`
- `roomeq-qa-coverage`
- `roomeq-qa-features`
- `roomeq-qa-synthetic`
- `convert-recording`

## Useful commands

```bash
cargo check -p autoeq
cargo clippy -p autoeq --no-deps
cargo test -p autoeq --lib
```

RoomEQ QA:

```bash
just qa-roomeq
just qa-roomeq-convergence
```

Targeted RoomEQ run:

```bash
cargo run --release --bin roomeq -- \
  --config crates/autoeq/tests/data/roomeq/test_config_stereo.json \
  --output /tmp/out.json
```

## Docs to keep in sync

- `CHANGELOG.md`
- `docs/roomeq_explained.md`
- `bin/roomeq/INPUT_FORMAT.md`
- `bin/roomeq/input_schema.json`
