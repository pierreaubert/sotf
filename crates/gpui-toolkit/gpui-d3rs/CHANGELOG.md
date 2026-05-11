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
