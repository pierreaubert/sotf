# D3.js Extended Modules Implementation Tasks

## Overview

Add 5 new D3.js modules to d3rs for increased parity with D3.js:
1. **d3-geo** - Geographic projections and paths
2. **d3-quadtree** - Spatial indexing data structure
3. **d3-delaunay** - Delaunay triangulation and Voronoi diagrams
4. **d3-timer** - Animation timing
5. **d3-transition** - Animated transitions

Note: **d3-ease** is already fully implemented in `src/ease/mod.rs`.

---

## Phase 1: d3-quadtree (~300 lines) ✅ COMPLETE

### Tasks
- [x] Create `src/quadtree/mod.rs` with QuadTree struct and Node enum
- [x] Implement `QuadTree::new()` and `QuadTree::from_data()`
- [x] Implement `.add()`, `.add_all()`, `.remove()`, `.remove_all()`
- [x] Implement `.find(x, y, radius)` for nearest neighbor search
- [x] Implement `.visit()` and `.visit_after()` traversals
- [x] Implement `.data()`, `.size()`, `.extent()`, `.copy()`
- [x] Write unit tests in module (13 tests passing)
- [x] Create `examples/quadtree_demo.rs`
- [x] Generate `golden/quadtree/quadtree.json` test data
- [x] Add golden tests to `tests/golden_tests.rs`
- [x] Add `pub mod quadtree;` to `src/lib.rs`

### Notes
- Our `remove()` works by coordinates, while D3.js requires object reference equality
- Full D3.js API parity with additional Rust idioms (generic data type support)

### Files
- `src/quadtree/mod.rs` (new)
- `examples/quadtree_demo.rs` (new)
- `golden/quadtree/quadtree.json` (new)
- `src/lib.rs` (modify)
- `tests/golden_tests.rs` (modify)

---

## Phase 2: d3-delaunay (~400 lines)

### Tasks
- [ ] Add `delaunator = "1.0"` to Cargo.toml
- [ ] Create `src/delaunay/mod.rs` wrapping delaunator crate
- [ ] Implement `Delaunay::new()` and `Delaunay::from_iter()`
- [ ] Implement `.find()`, `.neighbors()`, `.hull_polygon()`
- [ ] Implement `.triangle_polygons()`, `.render_to_path()`
- [ ] Create `src/delaunay/voronoi.rs` for Voronoi diagrams
- [ ] Implement `.voronoi(bounds)`, `.cell_polygon()`, `.cell_polygons()`
- [ ] Implement `.contains()`, `.neighbors()`, `.render_to_path()`
- [ ] Write unit tests
- [ ] Create `examples/delaunay_demo.rs`
- [ ] Generate `golden/delaunay/delaunay.json` test data
- [ ] Generate `golden/delaunay/voronoi.json` test data
- [ ] Add golden tests
- [ ] Add `pub mod delaunay;` to `src/lib.rs`
- [ ] (Optional) Add GPUI rendering in `src/shape/delaunay_render.rs`

### Files
- `Cargo.toml` (modify - add delaunator)
- `src/delaunay/mod.rs` (new)
- `src/delaunay/voronoi.rs` (new)
- `examples/delaunay_demo.rs` (new)
- `golden/delaunay/delaunay.json` (new)
- `golden/delaunay/voronoi.json` (new)
- `src/lib.rs` (modify)
- `tests/golden_tests.rs` (modify)

---

## Phase 3: d3-timer (~200 lines)

### Tasks
- [ ] Create `src/timer/mod.rs`
- [ ] Implement `now()` using `std::time::Instant`
- [ ] Implement `Timer` struct with callback, delay, time
- [ ] Implement `timer()`, `timeout()`, `interval()` constructors
- [ ] Implement `Timer::restart()` and `Timer::stop()`
- [ ] Implement `timer_flush()` for immediate execution
- [ ] Write unit tests
- [ ] Create `examples/timer_demo.rs`
- [ ] Add `pub mod timer;` to `src/lib.rs`

### Files
- `src/timer/mod.rs` (new)
- `examples/timer_demo.rs` (new)
- `src/lib.rs` (modify)

---

## Phase 4: d3-geo (~1000 lines, Extended)

### Tasks
- [ ] Create `src/geo/mod.rs` with Projection trait
- [ ] Create `src/geo/projection/mod.rs` for projection implementations
- [ ] Implement `Mercator` projection in `src/geo/projection/mercator.rs`
- [ ] Implement `Equirectangular` in `src/geo/projection/equirectangular.rs`
- [ ] Implement `Orthographic` in `src/geo/projection/orthographic.rs`
- [ ] Implement `Albers` in `src/geo/projection/albers.rs`
- [ ] Implement `AlbersUsa` in `src/geo/projection/albers_usa.rs`
- [ ] Implement `Stereographic` in `src/geo/projection/stereographic.rs`
- [ ] Implement `TransverseMercator` in `src/geo/projection/transverse_mercator.rs`
- [ ] Create `src/geo/path.rs` for GeoPath (GeoJSON to paths)
- [ ] Create `src/geo/circle.rs` for great circle generator
- [ ] Create `src/geo/graticule.rs` for lat/lon grid generator
- [ ] Implement utility functions: `geo_distance()`, `geo_area()`, `geo_bounds()`
- [ ] Implement `geo_interpolate()`, `geo_length()`, `geo_contains()`
- [ ] Write unit tests
- [ ] Create `examples/geo_demo.rs`
- [ ] Create `examples/geo_gpui_demo.rs` for GPUI map rendering
- [ ] Generate `golden/geo/projections.json` test data
- [ ] Add golden tests
- [ ] Add `pub mod geo;` to `src/lib.rs`

### Files
- `src/geo/mod.rs` (new)
- `src/geo/projection/mod.rs` (new)
- `src/geo/projection/mercator.rs` (new)
- `src/geo/projection/equirectangular.rs` (new)
- `src/geo/projection/orthographic.rs` (new)
- `src/geo/projection/albers.rs` (new)
- `src/geo/projection/albers_usa.rs` (new)
- `src/geo/projection/stereographic.rs` (new)
- `src/geo/projection/transverse_mercator.rs` (new)
- `src/geo/path.rs` (new)
- `src/geo/circle.rs` (new)
- `src/geo/graticule.rs` (new)
- `examples/geo_demo.rs` (new)
- `examples/geo_gpui_demo.rs` (new)
- `golden/geo/projections.json` (new)
- `src/lib.rs` (modify)
- `tests/golden_tests.rs` (modify)

---

## Phase 5: d3-transition (~400 lines, Full GPUI Integration)

Full GPUI integration with D3-style animation API.

### Tasks
- [ ] Create `src/transition/mod.rs` with Transition struct and core API
- [ ] Create `src/transition/tween.rs` with Tween trait and property tweens
- [ ] Create `src/transition/scheduler.rs` for animation scheduling
- [ ] Create `src/transition/interpolators.rs` (integrates with d3rs interpolate module)
- [ ] Implement `Transition::new()`, `.duration()`, `.delay()`, `.ease()`
- [ ] Implement `.attr()` and `.style()` for property animation
- [ ] Implement `.tween()` for custom tweening
- [ ] Implement `.on("start")`, `.on("end")`, `.on("interrupt")` event handlers
- [ ] Implement `.selection()` and `.remove()` methods
- [ ] Implement `TransitionManager` for tracking active transitions
- [ ] Integrate with existing d3rs ease module
- [ ] Integrate with GPUI's `Animation` and `AnimationExt` traits
- [ ] Create `examples/transition_gpui_demo.rs`
- [ ] Create `examples/transition_chart_demo.rs` for chart animations
- [ ] Add `pub mod transition;` to `src/lib.rs`
- [ ] Document GPUI integration pattern

### Files
- `src/transition/mod.rs` (new)
- `src/transition/tween.rs` (new)
- `src/transition/scheduler.rs` (new)
- `src/transition/interpolators.rs` (new)
- `examples/transition_gpui_demo.rs` (new)
- `examples/transition_chart_demo.rs` (new)
- `src/lib.rs` (modify)

---

## Testing Infrastructure

### Golden Test Generator Updates

Add to `golden/generate.js`:
```javascript
// Generate quadtree tests
function generateQuadtreeTests() { ... }

// Generate delaunay tests
function generateDelaunayTests() { ... }

// Generate geo projection tests
function generateGeoTests() { ... }
```

### Rust Test Harness Updates

Add to `tests/golden_tests.rs`:
```rust
#[test] fn test_quadtree_golden() { ... }
#[test] fn test_delaunay_golden() { ... }
#[test] fn test_geo_projections_golden() { ... }
```

---

## Dependencies

Add to `Cargo.toml`:
```toml
[dependencies]
delaunator = "1.0"  # For d3-delaunay
```

---

## Demo Examples Summary

| Module | Demo File | Type |
|--------|-----------|------|
| quadtree | `examples/quadtree_demo.rs` | Console |
| delaunay | `examples/delaunay_demo.rs` | Console |
| geo | `examples/geo_demo.rs` | Console |
| timer | `examples/timer_demo.rs` | Console |
| transition | `examples/transition_gpui_demo.rs` | GPUI |

---

## Estimated Lines of Code

| Module | Estimated LOC |
|--------|---------------|
| quadtree | ~300 |
| delaunay | ~400 |
| timer | ~200 |
| geo | ~800 |
| transition | ~300 |
| **Total** | **~2000** |

---

## Priority Order

1. **High**: d3-quadtree (enables fast spatial queries)
2. **High**: d3-delaunay (Rust crate exists, enables Voronoi viz)
3. **Medium**: d3-timer (needed for transitions)
4. **Medium**: d3-geo (large scope, specialized)
5. **Low**: d3-transition (GPUI-specific design needed)
