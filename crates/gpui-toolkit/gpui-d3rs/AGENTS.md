# gpui-d3rs (lib: `d3rs`, version: 0.6.0)

D3.js-inspired GPU-accelerated plotting library for GPUI.

## Key Features

- 2D and 3D GPU-accelerated charts
- Scales (linear, log, band, time, color)
- Axes, contours, shapes
- Spinorama speaker measurement visualization
- Delaunay triangulation
- Force-directed graphs
- Read `GPUI.md` at the project root before working on GPUI code

## Features

- `gpui` (default) - GPUI rendering integration
- `gpu-2d` (default) - 2D GPU-accelerated rendering
- `gpu-3d` - 3D surface rendering
- `spinorama` - Spinorama API integration for speaker data

## Binaries

- `d3rs-showcase` - Chart gallery
- `d3rs-spinorama` - Spinorama visualization demo

## Examples

15+ examples: scale, color, contour, delaunay, force, line, bar, scatter, etc.

## Testing

```bash
cargo test -p gpui-d3rs --no-default-features --tests   # Golden + unit tests
cargo test -p gpui-d3rs --lib                            # Unit tests only
cargo check -p gpui-d3rs && cargo clippy -p gpui-d3rs
```

## Example Architecture (Zero Duplication)

Each Observable example has three layers, all sharing the same compute code:

```
src/examples/pie_chart.rs    ← Pure computation (data → scales → layout → paths)
                               No GPUI, no test harness. Also serves as documentation.

tests/golden_tests.rs        ← Calls examples::pie_chart::compute(), validates vs golden JSON

bin/showcase/d3_examples/    ← Calls examples::pie_chart::compute(), renders via GPUI
```

**Available `src/examples/` modules:**

| Module | Observable Source | d3rs APIs Used |
|--------|-----------------|----------------|
| `hexbin` | `@d3/hexbin` | LogScale, Hexbin |
| `pie_chart` | `@d3/pie-chart` | Pie, Arc |
| `donut_chart` | `@d3/donut-chart` | Pie (inner radius), Arc |
| `line_chart` | `@d3/line-chart` | LinearScale, Curve (7 types) |
| `stacked_bar` | `@d3/stacked-bar-chart` | BandScale, Stack (Diverging) |
| `stacked_area` | `@d3/stacked-area-chart` | Stack, LinearScale, Curve |
| `streamgraph` | `@d3/streamgraph` | Stack (Wiggle + InsideOut) |
| `box_plot` | `@d3/box-plot` | BandScale, quantile stats |
| `chord` | `@d3/chord-diagram` | ChordLayout |
| `force_directed` | `@d3/force-directed-graph` | Simulation, ForceCenter, ForceManyBody |

## Golden Test Infrastructure

Three levels of D3.js compatibility testing in `golden/`:

### 1. Unit-level (`generate.js`)
Tests individual D3 primitives: scales, shapes, arrays, colors, interpolation.

### 2. Feature-level (`generate_examples.js`)
Tests D3 features in isolation: force simulation, hierarchy layouts, chord diagrams, etc.

### 3. Observable examples (`generate_observable_examples.js`) — NEW
Tests **complete visualization pipelines** from real Observable notebooks.
Each test captures the full chain: data -> scales -> layout -> color -> axes.

**Available examples:**

| Example | Observable URL | D3 Modules Tested | d3rs Modules Verified |
|---------|---------------|-------------------|----------------------|
| hexbin | `@d3/hexbin` | d3-hexbin, d3-scale | LogScale, Hexbin |
| streamgraph | `@d3/streamgraph` | d3-shape | Stack (Wiggle+InsideOut) |
| ortho_to_equirect | `@d3/orthographic-to-equirectangular` | d3-geo | Orthographic, Equirectangular |
| circle_packing | `@d3/zoomable-circle-packing` | d3-hierarchy | structure validation* |
| sunburst | `@d3/sunburst` | d3-hierarchy | structure validation* |
| versor | `@d3/versor-dragging` | d3-geo | Orthographic, quaternion math |
| box_plot | `@d3/box-plot` | d3-array, d3-scale | BandScale, statistics |
| force | `@d3/force-directed-graph` | d3-force | structure validation |
| sankey | `@d3/sankey` | d3-sankey | structure validation |
| chord | `@d3/chord-diagram` | d3-chord | structure validation |
| stacked_bar | `@d3/stacked-bar-chart` | d3-shape, d3-scale | BandScale, LinearScale |
| stacked_area | `@d3/stacked-area-chart` | d3-shape | Stack, area paths |
| line | `@d3/line-chart` | d3-shape | LinearScale, 7 curve types |
| pie | `@d3/pie-chart` | d3-shape | Pie, Arc |
| donut | `@d3/donut-chart` | d3-shape | Pie (inner radius), Arc |
| parallel_sets | `@d3/parallel-sets` | d3-sankey | flow conservation |

\* = d3rs does not yet implement pack/partition layouts; test validates golden file structure.

### Pipeline: Porting Observable Examples (7 Steps)

Every Observable example follows this disciplined pipeline. Complete all steps
before starting the next example.

#### Step 1: Capture the Observable Source

- Fetch the Observable notebook URL
- Extract the **exact D3.js code**: scales, generators, data transforms, color maps
- Identify **every D3 API call** used (e.g., `d3.scaleUtc`, `d3.area`, `d3.curveLinearClosed`)
- Note the exact dataset (CSV/JSON), column names, and any data transforms

#### Step 2: Generate Golden Data

**File**: `golden/generate_observable_examples.js` — add a new generator function

Rules:
- Data must be **deterministic** (`Math.sin(i*k)` or embed real data, never `Math.random()`)
- Prefer **real datasets** from `bin/showcase/data/` when available
- Use `.range()` not `.rangeRound()` for exact float comparison
- Capture **ALL intermediate outputs**:
  - Scale domains, ranges, and sample input→output pairs
  - Layout coordinates (every node/bin/slice position)
  - Generated paths (SVG path strings)
  - Axis tick values and formatted labels
  - Color assignments per data item
- Run: `cd golden && node generate_observable_examples.js <name>`
- Commit the JSON golden file

#### Step 3: Write the Compute Module

**File**: `src/examples/<name>.rs`

Rules:
- **Pure computation only** — no GPUI, no test harness, no rendering
- Use **d3rs APIs exclusively** — never hand-roll what d3rs provides:
  - `LinearScale`, `LogScale`, `TimeScale`, `BandScale` for scales
  - `Stack`, `Pie`, `Arc`, `Area`, `Curve` for shapes
  - `Hexbin`, `ChordLayout`, `Simulation` for layouts
  - `ColorScheme`, `SequentialScheme` for colors
  - `fetch::parse_csv` for data loading
  - `array::statistics::quantile_sorted` etc. for stats
- **Builder pattern** — chain `.domain().range().nice()` like D3.js
- **Functional style** — use closures for accessors: `.x(|d| ...).y0(|d| ...).y1(|d| ...)`
- Return a **result struct** with all computed geometry:
  - Positions, paths, colors, scale info, tick values
  - Everything the golden test and showcase need
- Include `default_data()` or `load_csv()`/`load_json()` for the real dataset
- Register in `src/examples/mod.rs`

#### Step 4: Write the Golden Test

**File**: `tests/golden_tests.rs` — add `test_observable_<name>()`

Rules:
- Load golden JSON file
- Call `examples::<name>::compute()` with the golden file's input data
- Assert **every intermediate value** against the golden data:
  - Scale outputs (input→output samples)
  - Layout positions (x, y, width, height for every element)
  - Path existence and structure
  - Bin counts, slice angles, node positions
  - Color assignments
- Use `approx_eq(expected, actual)` with tolerance 1e-6
- For non-deterministic algorithms (force simulation): verify convergence properties, not exact positions
- Run: `cargo test -p gpui-d3rs --no-default-features --test golden_tests test_observable_<name>`
- **Do NOT proceed to Step 5 until ALL golden assertions pass**

#### Step 5: Write the Showcase Renderer

**File**: `bin/showcase/showcase_modules/d3_examples/<name>.rs`

Rules:
- Call `examples::<name>::compute()` — **never duplicate computation logic**
- Use **d3rs path types** → `d3rs_path_to_gpui_simple()` for rendering
- Use `PathBuilder::stroke()` for lines, `PathBuilder::fill()` for areas
- Use **d3rs scales** for axis tick positions in the showcase too
- Use **d3rs color schemes** (not hardcoded hex arrays unless the Observable specifies exact colors)
- Include proper **axes**: tick labels, grid lines, axis lines
- Include **title**, **source URL**, **legend**
- Register in `d3_examples/mod.rs` and `main.rs` (DemoSection enum + label + render_content match)

#### Step 6: Review Checklist

For every example:
- [ ] Golden JSON captures ALL D3.js outputs (scales, paths, colors, ticks)
- [ ] `src/examples/<name>.rs` uses only d3rs APIs (no `format!("M {} {} L ...")`)
- [ ] Golden test asserts intermediate values, not just structure
- [ ] Showcase calls `compute()` and uses d3rs for rendering
- [ ] `cargo test -p gpui-d3rs --no-default-features --tests` — all pass
- [ ] `cargo clippy -p gpui-d3rs` — no new warnings
- [ ] Visual output matches the Observable example

#### Step 7: Update Documentation

- Update this table in AGENTS.md with new example
- Update `overview.rs` with clickable nav item
- If new d3rs API was added, update the gap analysis table

#### Key Principles

1. **Golden data is the source of truth** — if d3rs disagrees with D3.js, fix d3rs (not the test)
2. **No rendering before validation** — Step 4 must pass before Step 5 starts
3. **Use d3rs or implement in d3rs** — if an API is missing, add it to the library first
4. **Builder + functional > imperative** — `.x(|d| scale.scale(d.date)).y0(|d| ...)` not manual loops
5. **Real data over synthetic** — use CSV/JSON from `bin/showcase/data/` whenever possible
6. **One example at a time** — complete all 7 steps before starting the next example

#### Files Modified Per Example

| Step | File | Purpose |
|------|------|---------|
| 2 | `golden/generate_observable_examples.js` | JS generator |
| 2 | `golden/observable/<name>.json` | Golden data |
| 3 | `src/examples/<name>.rs` | Compute module |
| 3 | `src/examples/mod.rs` | Module registration |
| 4 | `tests/golden_tests.rs` | Golden test |
| 5 | `bin/showcase/.../<name>.rs` | Showcase renderer |
| 5 | `bin/showcase/.../d3_examples/mod.rs` | Module registration |
| 5 | `bin/showcase/main.rs` | DemoSection + menu |
| 7 | `AGENTS.md` | Documentation |
| 7 | `bin/showcase/.../overview.rs` | Clickable nav |

### Known Discrepancies

| Area | Issue | Status |
|------|-------|--------|
| Pie padAngle | d3rs distributes padding differently than D3.js | Test uses width tolerance |
| Stack InsideOut | Ordering algorithm differs slightly | Test verifies widths not positions |
| Hierarchy pack/partition | Not yet implemented in d3rs | Structure-only validation |
| Force simulation | Non-deterministic initial positions | Verify convergence, not positions |
| Line chart curves | D3.js emits native SVG curves (C/S), d3rs interpolates to L commands | Path structure validated, not exact strings |
| Line chart .nice() | D3.js uses .nice() domains, d3rs compute uses raw extent | Scale samples validated separately |

### Regenerating Golden Files

```bash
cd golden
npm install                              # First time only
npm run generate                         # Regenerate ALL golden files
node generate_observable_examples.js     # Observable examples only
node generate_observable_examples.js pie # Single example
```

## Notes

- API design inspired by D3.js but adapted for Rust and GPU rendering
- Used by `app-gpui` for real-time audio visualization
- Golden tests ensure 1:1 compatibility with D3.js v7.9.0
