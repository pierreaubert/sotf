# 0.7.0

## Breaking Changes

- `d3rs::fetch` parsing is now `Result`-first: `parse_csv`, `parse_tsv`,
  `parse_dsv`, `DsvParser::parse`, and `DsvParser::parse_rows` return
  structured `DsvParseError` values instead of silently returning empty data on
  malformed input.

## Features

- Added explicit lossy helpers (`parse_csv_lossy`, `parse_tsv_lossy`,
  `parse_dsv_lossy`, `DsvParser::parse_lossy`, and
  `DsvParser::parse_rows_lossy`) for D3-compatible demo paths.
- Added `ColumnPolicy::Strict` for header/row width validation plus empty and
  duplicate header rejection.
- DSV parsing now handles quoted newlines and CRLF input while reporting line,
  column, byte offset, and structured error kinds.

## Fixes

- `CsvOptions::default()` now matches `CsvOptions::new()` instead of disabling
  empty-line skipping and value trimming.

# 0.6.9

## Features

- `gpu3d::Lines3DElement`: new GPUI element rendering line / polygon scenes via CPU projection (`Camera3D::project_to_screen`) + `gpui::PathBuilder`. Same orbit / pan / zoom semantics as `Surface3DElement` through a shared `Lines3DState` (`Rc<RefCell<_>>`); parents wire mouse handlers to drive the embedded `OrbitControls`. Designed for sparse 3D scenes (~50 vertices) where a full wgpu pipeline would be overkill.

## Fixes

- **voronoi_airports example**: track `math-delaunay` API change — `triangles` and `halfedges` are now methods, not public fields. Updated callsites to use `.triangles()` / `.halfedges()`. Unblocks workspace compile.

# 0.6.8

## Features

- Added 3d lines with wpgu support

# 0.6.7

## Features

### Stroke dash array support for line rendering

- Added `StrokeDashArray` enum with predefined patterns (`Dotted`, `Dashed`,
  `DashDot`) and `Custom(Vec<f32>)` for arbitrary dash/gap sequences.
- Added `dash_array` field to `LineConfig` and a `.dash_array()` builder method.
- `render_line` now walks along line segments and splits them into dash/gap
  sub-segments when a pattern is set. The pattern state carries continuously
  across segments for seamless dashing.
- Re-exported `StrokeDashArray` from `shape::mod` and `lib.rs` prelude.

# 0.6.6

## Features

- Sphere gallery: GPU-rendered 3D sphere gallery with Metal shaders
- Legend rendering module (`legend/`)
- Voronoi stippling Observable example

## Fixes

- Fixed geo path clipping
- Fixed segfaults when data contains NaN
- iOS rendering support

# 0.6.5

## Features

- Sankey diagram layout engine
- 13 new Observable examples (ridgeline, sunburst, parallel sets, star map, etc.)
- Versor dragging for geo projections

## Fixes

- Voronoi rendering fixes
- NaN/error tolerance in plot rendering
- Dead code cleanup and clippy lints

# 0.6.4

## Features

- Observable examples framework (hexbin, pie, donut, line, stacked bar/area, streamgraph, chord, force-directed, box plot)
- Chord diagram layout
- Hexbin aggregation module

## Fixes

- Force simulation clamping during interpolation
- Log scale improvements
- Stack layout fixes

# 0.6.1

## Features

- Upgraded wgpu to latest version
- Split autoeq UI from UI Kit into standalone crate

## Fixes

- Clippy lints and formatting cleanup

# 0.6.0

- Initial release after crate reorganization (renamed from internal paths to `gpui-d3rs`)
- D3.js-inspired scales (linear, log, band, time, color)
- Shape rendering (line, bar, scatter, arc, pie, area, contour, heatmap)
- GPU-accelerated 2D and 3D rendering
- Axis and grid rendering
- Force-directed graph layout
- Delaunay triangulation
- Golden test infrastructure for D3.js compatibility
