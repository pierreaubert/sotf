# feat: Create gpui-px Crate (High-Level Chart API)

## Overview

Create a new crate `gpui-px` that provides a Plotly Express-style high-level API for creating charts. This crate **depends entirely on `gpui-d3rs`** for all low-level visualization primitives.

**Architecture**:
```
gpui-px (high-level API)
    │
    └── gpui-d3rs (low-level D3.js-like primitives)
            │
            └── gpui (rendering framework)
```

**Scope**: `gpui-px` is a thin convenience layer. When features are missing from `gpui-d3rs`, we track them in a separate "d3rs feature parity" plan—NOT in this crate.

## Problem Statement

Creating charts with `gpui-d3rs` requires 50-100+ lines:
- Manual scale creation with explicit domains/ranges
- Manual data preparation into specific structs
- Manual composition of axes, grids, marks

**Goal**: Enable creating complete charts in **3-5 lines** using `gpui-px`.

---

## Proposed Solution

### New Crate Structure

```
gpui-px/
├── Cargo.toml
├── src/
│   ├── lib.rs           # Re-exports
│   ├── error.rs         # ChartError type
│   ├── scatter.rs       # px::scatter()
│   ├── line.rs          # px::line()
│   └── bar.rs           # px::bar()
└── examples/
    └── basic_charts.rs
```

### Cargo.toml

```toml
[package]
name = "gpui-px"
version = "0.1.0"
edition = "2021"
description = "High-level Plotly Express-style chart API built on gpui-d3rs"

[dependencies]
gpui-d3rs = { path = "../gpui-d3rs", features = ["gpui"] }
gpui = { git = "https://github.com/zed-industries/zed" }
thiserror = "2"
```

---

## Technical Approach

### Design Principles

1. **`gpui-px` is ONLY a convenience wrapper** - All rendering logic lives in `gpui-d3rs`
2. **Never duplicate d3rs functionality** - If d3rs can't do it, track as d3rs enhancement
3. **Concrete types** - Accept `&[f64]` not traits
4. **Return Result** - Proper error handling
5. **Separate spec from render** - `build()` returns validated spec, `render()` produces element

### d3rs Features Used

| px feature | d3rs module used |
|------------|------------------|
| Scatter points | `shape::render_scatter`, `ScatterConfig`, `ScatterPoint` |
| Line charts | `shape::render_line`, `LineConfig`, `LinePoint`, `CurveType` |
| Bar charts | `shape::render_bars`, `BarConfig`, `BarDatum` |
| X/Y axes | `axis::render_axis`, `AxisConfig`, `AxisOrientation` |
| Grid | `grid::render_grid`, `GridConfig` |
| Linear scales | `scale::LinearScale`, `Scale` trait |
| Band scales | `scale::BandScale` |
| Colors | `color::D3Color`, `ColorScheme` |
| Auto ticks | `scale::generate_linear_ticks` |

### d3rs Features That May Be Missing

Track these in `plans/d3rs-feature-parity.md` (separate plan):

| px need | d3rs status | action |
|---------|-------------|--------|
| Auto domain padding | Not exposed | Add `auto_domain()` to d3rs |
| Legend rendering | `legend/` module exists but unclear | Verify API |
| Time scales | `time/scale.rs` exists | Verify API |

---

## Implementation

### Phase 1: Core Module (~50 LOC)

```rust
// gpui-px/src/lib.rs

pub mod error;
mod scatter;
mod line;
mod bar;

pub use error::ChartError;
pub use scatter::{scatter, ScatterBuilder, ScatterChart};
pub use line::{line, LineBuilder, LineChart};
pub use bar::{bar, BarBuilder, BarChart};

// Re-export d3rs types users might need
pub use gpui_d3rs::color::D3Color;
pub use gpui_d3rs::shape::CurveType;
```

```rust
// gpui-px/src/error.rs

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChartError {
    #[error("insufficient data: need at least 1 point, got {0}")]
    InsufficientData(usize),
    #[error("mismatched data lengths: x={0}, y={1}")]
    MismatchedLengths(usize, usize),
}

/// Compute domain with 5% padding
/// NOTE: This may move to gpui-d3rs in future
pub(crate) fn auto_domain(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 1.0);
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let padding = (max - min).abs() * 0.05;
    (min - padding, max + padding)
}
```

### Phase 2: Scatter (~120 LOC)

```rust
// gpui-px/src/scatter.rs

use crate::error::{auto_domain, ChartError};
use gpui::*;
use gpui_d3rs::prelude::*;

/// Create a scatter plot from x and y data slices
///
/// # Example
/// ```rust
/// use gpui_px::scatter;
///
/// let chart = scatter(&x_values, &y_values)
///     .title("My Chart")
///     .build()?
///     .render(window, cx);
/// ```
pub fn scatter(x: &[f64], y: &[f64]) -> ScatterBuilder {
    ScatterBuilder {
        x: x.to_vec(),
        y: y.to_vec(),
        title: None,
        x_label: None,
        y_label: None,
        width: 600.0,
        height: 400.0,
        color: D3Color::from_hex(0x1f77b4),
        point_radius: 6.0,
        opacity: 0.7,
        x_range: None,
        y_range: None,
        show_grid: true,
    }
}

pub struct ScatterBuilder {
    x: Vec<f64>,
    y: Vec<f64>,
    title: Option<String>,
    x_label: Option<String>,
    y_label: Option<String>,
    width: f32,
    height: f32,
    color: D3Color,
    point_radius: f32,
    opacity: f32,
    x_range: Option<(f64, f64)>,
    y_range: Option<(f64, f64)>,
    show_grid: bool,
}

impl ScatterBuilder {
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn x_label(mut self, label: impl Into<String>) -> Self {
        self.x_label = Some(label.into());
        self
    }

    pub fn y_label(mut self, label: impl Into<String>) -> Self {
        self.y_label = Some(label.into());
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn color(mut self, color: D3Color) -> Self {
        self.color = color;
        self
    }

    pub fn point_radius(mut self, radius: f32) -> Self {
        self.point_radius = radius;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn x_range(mut self, min: f64, max: f64) -> Self {
        self.x_range = Some((min, max));
        self
    }

    pub fn y_range(mut self, min: f64, max: f64) -> Self {
        self.y_range = Some((min, max));
        self
    }

    pub fn grid(mut self, show: bool) -> Self {
        self.show_grid = show;
        self
    }

    /// Build the chart specification (validates data)
    pub fn build(self) -> Result<ScatterChart, ChartError> {
        if self.x.len() != self.y.len() {
            return Err(ChartError::MismatchedLengths(self.x.len(), self.y.len()));
        }
        if self.x.is_empty() {
            return Err(ChartError::InsufficientData(0));
        }
        Ok(ScatterChart { spec: self })
    }
}

pub struct ScatterChart {
    spec: ScatterBuilder,
}

impl ScatterChart {
    /// Render the chart to a GPUI element using gpui-d3rs primitives
    pub fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let s = self.spec;

        // Layout
        let margin_top = if s.title.is_some() { 40.0 } else { 20.0 };
        let margin_right = 20.0;
        let margin_bottom = 60.0;
        let margin_left = 60.0;
        let plot_width = s.width - margin_left - margin_right;
        let plot_height = s.height - margin_top - margin_bottom;

        // Compute domains (use d3rs scales)
        let x_domain = s.x_range.unwrap_or_else(|| auto_domain(&s.x));
        let y_domain = s.y_range.unwrap_or_else(|| auto_domain(&s.y));

        // Create d3rs scales
        let x_scale = LinearScale::new()
            .domain(x_domain.0, x_domain.1)
            .range(0.0, plot_width as f64);

        let y_scale = LinearScale::new()
            .domain(y_domain.0, y_domain.1)
            .range(plot_height as f64, 0.0);

        // Build d3rs scatter points
        let points: Vec<ScatterPoint> = s.x.iter()
            .zip(s.y.iter())
            .map(|(&x, &y)| ScatterPoint { x, y })
            .collect();

        // Configure d3rs scatter
        let config = ScatterConfig::new()
            .fill_color(s.color)
            .point_radius(s.point_radius)
            .opacity(s.opacity);

        // Render using d3rs
        let marks = render_scatter(&x_scale, &y_scale, &points, &config);

        // Build d3rs axes
        let x_axis_config = AxisConfig::bottom();
        let y_axis_config = AxisConfig::left();
        let x_axis = render_axis(&x_scale, &x_axis_config);
        let y_axis = render_axis(&y_scale, &y_axis_config);

        // Build d3rs grid
        let grid = render_grid(&x_scale, &y_scale, &GridConfig::default());

        // Compose layout (pure GPUI)
        v_flex()
            .w(px(s.width))
            .h(px(s.height))
            .when_some(s.title, |el, title| {
                el.child(
                    div()
                        .h(px(margin_top))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(title)
                )
            })
            .child(
                h_flex()
                    .child(
                        div()
                            .w(px(margin_left))
                            .h(px(plot_height))
                            .child(y_axis)
                    )
                    .child(
                        div()
                            .relative()
                            .w(px(plot_width))
                            .h(px(plot_height))
                            .when(s.show_grid, |el| el.child(grid))
                            .child(marks)
                    )
            )
            .child(
                div()
                    .h(px(margin_bottom))
                    .ml(px(margin_left))
                    .child(x_axis)
            )
    }
}
```

### Phase 3: Line Chart (~100 LOC)

```rust
// gpui-px/src/line.rs

use crate::error::{auto_domain, ChartError};
use gpui::*;
use gpui_d3rs::prelude::*;

pub fn line(x: &[f64], y: &[f64]) -> LineBuilder {
    LineBuilder {
        x: x.to_vec(),
        y: y.to_vec(),
        title: None,
        x_label: None,
        y_label: None,
        width: 600.0,
        height: 400.0,
        stroke_color: D3Color::from_hex(0x1f77b4),
        stroke_width: 2.0,
        markers: false,
        curve: CurveType::Linear,
        x_range: None,
        y_range: None,
        show_grid: true,
    }
}

pub struct LineBuilder {
    // ... similar fields
}

impl LineBuilder {
    // ... similar builder methods

    pub fn markers(mut self, show: bool) -> Self {
        self.markers = show;
        self
    }

    pub fn curve(mut self, curve: CurveType) -> Self {
        self.curve = curve;
        self
    }

    pub fn build(self) -> Result<LineChart, ChartError> { ... }
}

pub struct LineChart { spec: LineBuilder }

impl LineChart {
    pub fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let s = self.spec;

        // ... same layout calculation

        // Build d3rs line points
        let points: Vec<LinePoint> = s.x.iter()
            .zip(s.y.iter())
            .map(|(&x, &y)| LinePoint::new(x, y))
            .collect();

        // Configure d3rs line
        let config = LineConfig::new()
            .stroke_color(s.stroke_color)
            .stroke_width(s.stroke_width)
            .curve(s.curve)
            .show_points(s.markers);

        // Render using d3rs
        let marks = render_line(&x_scale, &y_scale, &points, &config);

        // ... same composition
    }
}
```

### Phase 4: Bar Chart (~100 LOC)

```rust
// gpui-px/src/bar.rs

use crate::error::ChartError;
use gpui::*;
use gpui_d3rs::prelude::*;
use gpui_d3rs::scale::BandScale;

pub fn bar(categories: &[&str], values: &[f64]) -> BarBuilder {
    BarBuilder {
        categories: categories.iter().map(|s| s.to_string()).collect(),
        values: values.to_vec(),
        title: None,
        x_label: None,
        y_label: None,
        width: 600.0,
        height: 400.0,
        color: D3Color::from_hex(0x1f77b4),
        y_range: None,
        show_grid: true,
    }
}

pub struct BarBuilder { ... }

impl BarBuilder {
    pub fn build(self) -> Result<BarChart, ChartError> { ... }
}

pub struct BarChart { spec: BarBuilder }

impl BarChart {
    pub fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let s = self.spec;

        // Use d3rs BandScale for categorical x-axis
        let x_scale = BandScale::new()
            .domain(s.categories.clone())
            .range(0.0, plot_width as f64)
            .padding(0.1);

        // Build d3rs bar data
        let bars: Vec<BarDatum> = s.categories.iter()
            .zip(s.values.iter())
            .map(|(cat, &val)| BarDatum { category: cat.clone(), value: val })
            .collect();

        // Configure d3rs bars
        let config = BarConfig::new()
            .fill_color(s.color);

        // Render using d3rs
        let marks = render_bars(&x_scale, &y_scale, &bars, &config);

        // ... same composition
    }
}
```

---

## Acceptance Criteria

### Functional Requirements

- [ ] New crate `gpui-px` builds successfully
- [ ] `gpui-px` depends on `gpui-d3rs` (no direct GPUI rendering except layout)
- [ ] `px::scatter(&x, &y)` produces complete chart
- [ ] `px::line(&x, &y)` produces complete chart
- [ ] `px::bar(&categories, &values)` produces complete chart
- [ ] All builders have `.title()`, `.x_label()`, `.y_label()`, `.size()`
- [ ] All charts return `Result<Chart, ChartError>`

### Non-Functional Requirements

- [ ] Total new code <400 LOC
- [ ] Simple chart in 3-5 lines of code
- [ ] No panics - all errors handled via Result
- [ ] Zero rendering code in gpui-px (all delegated to d3rs)

### Quality Gates

- [ ] `cargo check -p gpui-px`
- [ ] `cargo clippy -p gpui-px`
- [ ] `cargo doc -p gpui-px`
- [ ] Example compiles and runs

---

## Usage Examples

```rust
use gpui_px::{scatter, line, bar, D3Color, CurveType};

// Scatter plot - 3 lines
let chart = scatter(&x_data, &y_data)
    .title("GDP vs Life Expectancy")
    .build()?
    .render(window, cx);

// Line chart with markers - 4 lines
let chart = line(&dates, &prices)
    .title("Stock Price")
    .markers(true)
    .curve(CurveType::Monotone)
    .build()?
    .render(window, cx);

// Bar chart - 3 lines
let chart = bar(&["Q1", "Q2", "Q3", "Q4"], &[100.0, 150.0, 120.0, 180.0])
    .title("Quarterly Sales")
    .build()?
    .render(window, cx);
```

---

## What's NOT in Scope

### Deferred to v2
- Color encoding (multiple colors based on data)
- Size encoding (point size based on data)
- Legends
- Multiple series (e.g., `line_multi()`)
- Faceting

### Out of Scope (belongs in gpui-d3rs)
- New chart types (pie, area, heatmap)
- Time scales
- Tooltips/hover
- Animation
- Any new rendering primitives

When we need d3rs features that don't exist, add them to: `plans/d3rs-feature-parity.md`

---

## Files to Create

1. `gpui-px/Cargo.toml`
2. `gpui-px/src/lib.rs` (~20 LOC)
3. `gpui-px/src/error.rs` (~30 LOC)
4. `gpui-px/src/scatter.rs` (~120 LOC)
5. `gpui-px/src/line.rs` (~100 LOC)
6. `gpui-px/src/bar.rs` (~100 LOC)
7. `gpui-px/examples/basic_charts.rs` (~50 LOC)

**Total: ~420 LOC**

---

## d3rs Features to Verify

Before implementation, verify these d3rs features work as expected:

- [ ] `render_scatter()` with `ScatterConfig`
- [ ] `render_line()` with `LineConfig` and `CurveType`
- [ ] `render_bars()` with `BarConfig`
- [ ] `render_axis()` with `AxisConfig::bottom()` / `left()`
- [ ] `render_grid()` with `GridConfig`
- [ ] `LinearScale` domain/range
- [ ] `BandScale` for categorical data

If any are missing or broken, create issues in `plans/d3rs-feature-parity.md`.

---

## References

### Internal
- gpui-d3rs lib: `gpui-d3rs/src/lib.rs`
- d3rs scales: `gpui-d3rs/src/scale/mod.rs`
- d3rs shapes: `gpui-d3rs/src/shape/mod.rs`
- d3rs axis: `gpui-d3rs/src/axis/mod.rs`
- d3rs grid: `gpui-d3rs/src/grid/mod.rs`

### External
- [Plotly Express API](https://plotly.com/python-api-reference/plotly.express.html)
