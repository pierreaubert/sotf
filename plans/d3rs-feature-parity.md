# d3rs Feature Parity Tracking

## Overview

This document tracks features needed in `gpui-d3rs` to achieve parity with D3.js and support higher-level APIs like `gpui-px`.

**Principle**: All visualization primitives belong in `gpui-d3rs`. Higher-level APIs (like `gpui-px`) should ONLY compose these primitives, never implement rendering logic.

---

## Current d3rs Module Coverage

### Implemented (verified)

| D3.js Module | d3rs Status | Notes |
|--------------|-------------|-------|
| d3-scale | ✅ Complete | Linear, Log, Pow, Symlog, Quantize, Quantile, Threshold, Ordinal, Band, Point |
| d3-axis | ✅ Complete | Four orientations, tick formatting, titles |
| d3-shape (basic) | ✅ Complete | Line, Bar, Scatter, Area, Arc, Pie |
| d3-shape (curves) | ✅ Complete | Linear, Step, Basis, Cardinal, Monotone, Natural |
| d3-color | ✅ Complete | RGB, HSL, interpolation, schemes |
| d3-array | ✅ Complete | Statistics, search, binning, transforms |
| d3-interpolate | ✅ Complete | Number, color, transform, string, zoom |
| d3-format | ✅ Complete | SI prefixes, locales |
| d3-contour | ✅ Complete | Marching squares, density |
| d3-time | ⚠️ Partial | Intervals exist, time scale needs verification |

### Missing or Needs Enhancement

| Feature | D3.js Reference | Priority | Notes |
|---------|-----------------|----------|-------|
| `auto_domain()` utility | d3.extent + padding | High | Needed by gpui-px |
| Legend rendering API | d3-legend (plugin) | Medium | `legend/` exists but unclear |
| Time scale | d3-scale-time | Medium | Verify `time/scale.rs` works |
| Tooltips | d3-tip (plugin) | Low | Interactive feature |
| Brush selection | d3-brush | Low | `brush/` module exists |
| Zoom behavior | d3-zoom | Low | `zoom/` module exists |

---

## Feature Requests from gpui-px

### High Priority (needed for px v1)

#### 1. `auto_domain()` / `extent_padded()`

**Description**: Calculate domain from data with optional padding.

**D3 equivalent**:
```javascript
const extent = d3.extent(data);
const padding = (extent[1] - extent[0]) * 0.05;
return [extent[0] - padding, extent[1] + padding];
```

**Proposed d3rs API**:
```rust
// In gpui-d3rs/src/array/statistics.rs
pub fn extent_padded(values: &[f64], padding_fraction: f64) -> (f64, f64) {
    let (min, max) = extent(values);
    let padding = (max - min).abs() * padding_fraction;
    (min - padding, max + padding)
}
```

**Status**: Not implemented
**Effort**: Small (~10 LOC)

---

#### 2. Verify `render_axis()` with titles

**Description**: Confirm axis titles work correctly.

**Current API**:
```rust
let config = AxisConfig::bottom().title("X Axis Label");
let axis = render_axis(&scale, &config);
```

**Status**: Needs verification
**Effort**: Testing only

---

#### 3. Verify `BandScale` for bar charts

**Description**: Confirm BandScale works with `render_bars()`.

**Current API**:
```rust
let scale = BandScale::new()
    .domain(vec!["A", "B", "C"])
    .range(0.0, 400.0)
    .padding(0.1);
```

**Status**: Needs verification
**Effort**: Testing only

---

### Medium Priority (needed for px v2)

#### 4. Legend Component

**Description**: Render legends for color/shape mappings.

**D3 equivalent**: d3-legend plugin

**Proposed d3rs API**:
```rust
let legend = Legend::new()
    .title("Categories")
    .items(vec![
        ("Series A", D3Color::from_hex(0x1f77b4)),
        ("Series B", D3Color::from_hex(0xff7f0e)),
    ])
    .orientation(LegendOrientation::Vertical);

let element = render_legend(&legend);
```

**Status**: Module exists (`legend/`), needs API verification
**Effort**: Medium

---

#### 5. Multiple Line Series

**Description**: Render multiple lines with different colors.

**Current**: `render_line()` handles single line

**Enhancement needed**:
```rust
// Option A: Multiple calls
render_line(&scale_x, &scale_y, &series_a, &config_a);
render_line(&scale_x, &scale_y, &series_b, &config_b);

// Option B: render_lines() for multiple
render_lines(&scale_x, &scale_y, &[
    (&series_a, &config_a),
    (&series_b, &config_b),
]);
```

**Status**: May already work with Option A
**Effort**: Verification or small enhancement

---

### Low Priority (future)

#### 6. Time Scale Verification

**Description**: Verify time scales work for temporal data.

**File**: `gpui-d3rs/src/time/scale.rs`

**Status**: Exists, needs testing

---

#### 7. Area Chart Rendering

**Description**: `render_area()` for area charts.

**Current**: `area.rs` has data structures, unclear if GPUI rendering exists

**Status**: Needs verification

---

#### 8. Stacked Bar Charts

**Description**: Support for stacked/grouped bars.

**Current**: `stack.rs` has stack layout

**Status**: Needs integration with `render_bars()`

---

## How to Use This Document

1. **When gpui-px needs a feature**: Check here first
2. **If feature exists**: Verify it works, update status
3. **If feature missing**: Add to this list with priority
4. **Before implementing in gpui-px**: Implement in d3rs first, then use in px

---

## Related Plans

- `plans/feat-gpui-px-crate.md` - High-level px API plan
