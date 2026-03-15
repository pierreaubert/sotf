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

### How to Add a New Observable Example

1. **Find the Observable notebook** (e.g. `https://observablehq.com/@d3/treemap`)

2. **Add d3 package** if needed:
   ```bash
   cd golden && npm install d3-xxx
   ```

3. **Add JS generator** in `golden/generate_observable_examples.js`:
   - Deterministic data (`Math.sin()`, not `Math.random()`)
   - Use `.range()` not `.rangeRound()` for exact float comparison
   - Capture ALL intermediate values (scales, layout coords, paths)
   ```bash
   cd golden && node generate_observable_examples.js treemap
   ```

4. **Add plot module** in `src/examples/treemap.rs`:
   - Pure computation: `pub fn compute(data) -> TreemapResult`
   - No GPUI, no test harness
   - Include `default_data()` for documentation/demo
   - Register in `src/examples/mod.rs`

5. **Add golden test** in `tests/golden_tests.rs`:
   - Call `examples::treemap::compute()` with golden file data
   - Compare output against golden JSON values
   ```bash
   cargo test -p gpui-d3rs --no-default-features --tests test_observable_treemap
   ```

6. **Add showcase render** in `bin/showcase/showcase_modules/d3_examples/`:
   - Call `examples::treemap::compute()`, render result with GPUI

### Known Discrepancies

| Area | Issue | Status |
|------|-------|--------|
| Pie padAngle | d3rs distributes padding differently than D3.js | Test uses width tolerance |
| Stack InsideOut | Ordering algorithm differs slightly | Test verifies widths not positions |
| Hierarchy pack/partition | Not yet implemented in d3rs | Structure-only validation |
| Force simulation | Non-deterministic initial positions | Verify convergence, not positions |

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
