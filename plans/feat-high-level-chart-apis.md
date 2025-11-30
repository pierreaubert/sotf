# feat: Add High-Level Chart API (px module)

## Overview

Add a simple, Plotly Express-style high-level API to `gpui-d3rs` that enables creating complete charts in 3-5 lines of code using concrete data types.

**Scope**: px-style API only. No Vega-Lite, no DataFrame abstraction, no transforms.

## Problem Statement

Currently, `gpui-d3rs` requires 100+ lines for a simple chart (see `spinorama_demo.rs`):
- Manual scale creation with explicit domains/ranges
- Manual data preparation into specific structs
- Manual composition of axes, grids, marks, legends
- Manual layout and positioning

**Goal**: Enable creating complete charts in **3-5 lines** with automatic inference.

## Proposed Solution

### Module Structure

```
src/
├── chart/
│   ├── mod.rs           # Re-exports, auto_domain() helper
│   ├── scatter.rs       # px::scatter()
│   ├── line.rs          # px::line()
│   └── bar.rs           # px::bar()
```

No `data.rs`, no `inference.rs`, no `compose.rs`. Inline everything.

---

## Technical Approach

### Design Principles

1. **Concrete types over abstractions** - Accept `&[f64]` not `ChartData` trait
2. **Inline layout** - No `ChartComposer`, each chart handles its own layout
3. **Return Result** - Proper error handling, no panics
4. **Separate spec from render** - `build()` returns spec, `render()` produces element

### Phase 1: Minimal Scatter (~150 LOC)

```rust
// src/chart/mod.rs

pub mod scatter;
pub mod line;
pub mod bar;

pub use scatter::scatter;
pub use line::line;
pub use bar::bar;

/// Error type for chart operations
#[derive(Debug, thiserror::Error)]
pub enum ChartError {
    #[error("insufficient data: need at least 1 point, got {0}")]
    InsufficientData(usize),
    #[error("mismatched data lengths: x={0}, y={1}")]
    MismatchedLengths(usize, usize),
}

/// Compute domain with 5% padding
pub fn auto_domain(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 1.0);
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let padding = (max - min).abs() * 0.05;
    (min - padding, max + padding)
}
```

```rust
// src/chart/scatter.rs

use crate::chart::{auto_domain, ChartError};
use crate::scale::LinearScale;
use crate::shape::{render_scatter, ScatterConfig, ScatterPoint};
use crate::axis::{render_axis, AxisConfig};
use crate::grid::{render_grid, GridConfig};
use crate::color::D3Color;
use gpui::*;

/// Create a scatter plot from x and y data slices
///
/// # Example
/// ```rust
/// let chart = px::scatter(&x_values, &y_values)
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
    /// Render the chart to a GPUI element
    pub fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let s = self.spec;

        // Layout constants
        let margin_top = if s.title.is_some() { 40.0 } else { 20.0 };
        let margin_right = 20.0;
        let margin_bottom = 60.0;
        let margin_left = 60.0;

        let plot_width = s.width - margin_left - margin_right;
        let plot_height = s.height - margin_top - margin_bottom;

        // Compute scales
        let x_domain = s.x_range.unwrap_or_else(|| auto_domain(&s.x));
        let y_domain = s.y_range.unwrap_or_else(|| auto_domain(&s.y));

        let x_scale = LinearScale::new()
            .domain(x_domain.0, x_domain.1)
            .range(0.0, plot_width as f64);

        let y_scale = LinearScale::new()
            .domain(y_domain.0, y_domain.1)
            .range(plot_height as f64, 0.0);

        // Build points
        let points: Vec<ScatterPoint> = s.x.iter()
            .zip(s.y.iter())
            .map(|(&x, &y)| ScatterPoint { x, y })
            .collect();

        let config = ScatterConfig::new()
            .fill_color(s.color)
            .point_radius(s.point_radius)
            .opacity(s.opacity);

        // Render components
        let marks = render_scatter(&x_scale, &y_scale, &points, &config);

        let x_axis_config = AxisConfig::bottom()
            .when_some(s.x_label.as_ref(), |c, label| c.title(label));
        let y_axis_config = AxisConfig::left()
            .when_some(s.y_label.as_ref(), |c, label| c.title(label));

        let x_axis = render_axis(&x_scale, &x_axis_config);
        let y_axis = render_axis(&y_scale, &y_axis_config);

        // Compose chart
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
                            .when(s.show_grid, |el| {
                                el.child(render_grid(&x_scale, &y_scale, &GridConfig::default()))
                            })
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

### Phase 2: Line Chart (~100 LOC additional)

```rust
// src/chart/line.rs

/// Create a line chart from x and y data slices
///
/// # Example
/// ```rust
/// let chart = px::line(&x_values, &y_values)
///     .title("Time Series")
///     .markers(true)
///     .build()?
///     .render(window, cx);
/// ```
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
    x: Vec<f64>,
    y: Vec<f64>,
    title: Option<String>,
    x_label: Option<String>,
    y_label: Option<String>,
    width: f32,
    height: f32,
    stroke_color: D3Color,
    stroke_width: f32,
    markers: bool,
    curve: CurveType,
    x_range: Option<(f64, f64)>,
    y_range: Option<(f64, f64)>,
    show_grid: bool,
}

impl LineBuilder {
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

    pub fn stroke_color(mut self, color: D3Color) -> Self {
        self.stroke_color = color;
        self
    }

    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width;
        self
    }

    pub fn markers(mut self, show: bool) -> Self {
        self.markers = show;
        self
    }

    pub fn curve(mut self, curve: CurveType) -> Self {
        self.curve = curve;
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

    pub fn build(self) -> Result<LineChart, ChartError> {
        if self.x.len() != self.y.len() {
            return Err(ChartError::MismatchedLengths(self.x.len(), self.y.len()));
        }
        if self.x.is_empty() {
            return Err(ChartError::InsufficientData(0));
        }
        Ok(LineChart { spec: self })
    }
}

pub struct LineChart {
    spec: LineBuilder,
}

impl LineChart {
    pub fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // Same layout pattern as scatter, using render_line() instead
        todo!()
    }
}
```

### Phase 3: Bar Chart (~100 LOC additional)

```rust
// src/chart/bar.rs

/// Create a bar chart from categories and values
///
/// # Example
/// ```rust
/// let chart = px::bar(&["A", "B", "C"], &[10.0, 20.0, 15.0])
///     .title("Sales by Category")
///     .build()?
///     .render(window, cx);
/// ```
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
        bar_gap: 0.1,
        y_range: None,
        show_grid: true,
    }
}

pub struct BarBuilder {
    categories: Vec<String>,
    values: Vec<f64>,
    title: Option<String>,
    x_label: Option<String>,
    y_label: Option<String>,
    width: f32,
    height: f32,
    color: D3Color,
    bar_gap: f32,
    y_range: Option<(f64, f64)>,
    show_grid: bool,
}

impl BarBuilder {
    // Similar builder methods...

    pub fn build(self) -> Result<BarChart, ChartError> {
        if self.categories.len() != self.values.len() {
            return Err(ChartError::MismatchedLengths(self.categories.len(), self.values.len()));
        }
        if self.categories.is_empty() {
            return Err(ChartError::InsufficientData(0));
        }
        Ok(BarChart { spec: self })
    }
}

pub struct BarChart {
    spec: BarBuilder,
}

impl BarChart {
    pub fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // Uses BandScale for x, LinearScale for y
        todo!()
    }
}
```

---

## Acceptance Criteria

### Functional Requirements

- [ ] `px::scatter(&x, &y)` produces complete chart with axes and grid
- [ ] `px::line(&x, &y)` produces complete chart with axes and grid
- [ ] `px::bar(&categories, &values)` produces complete chart with axes and grid
- [ ] All builders have `.title()`, `.x_label()`, `.y_label()`, `.size()`
- [ ] All charts auto-compute domain with 5% padding
- [ ] All charts return `Result<Chart, ChartError>`

### Non-Functional Requirements

- [ ] Total new code <400 LOC
- [ ] Simple chart in 3-5 lines of code
- [ ] Charts render in <16ms for 1000 data points
- [ ] No panics - all errors handled via Result

### Quality Gates

- [ ] All existing tests pass
- [ ] New tests for each chart type
- [ ] Example in `examples/chart_demo.rs`
- [ ] `cargo clippy` passes
- [ ] `cargo doc` builds

---

## Usage Examples

```rust
use gpui_d3rs::chart::{scatter, line, bar};

// Scatter plot - 3 lines
let chart = scatter(&x_data, &y_data)
    .title("GDP vs Life Expectancy")
    .build()?
    .render(window, cx);

// Line chart with markers - 4 lines
let chart = line(&dates, &prices)
    .title("Stock Price")
    .markers(true)
    .build()?
    .render(window, cx);

// Bar chart - 3 lines
let chart = bar(&["Q1", "Q2", "Q3", "Q4"], &[100.0, 150.0, 120.0, 180.0])
    .title("Quarterly Sales")
    .build()?
    .render(window, cx);
```

---

## What's NOT in Scope (Deferred)

- **Vega-Lite API** - Build if users request grammar-based composition
- **DataFrame abstraction** - Use concrete slices; add trait when needed
- **Color encoding** - Single color per chart; add categorical colors in v2
- **Size encoding** - Fixed point size; add data-driven sizing in v2
- **Legends** - Add when color/size encoding is added
- **Transforms** - Users filter/aggregate before calling chart functions
- **Multiple series** - Add `line_multi(&[Series])` variant in v2

---

## Files to Create

1. `gpui-d3rs/src/chart/mod.rs` (~30 LOC)
2. `gpui-d3rs/src/chart/scatter.rs` (~150 LOC)
3. `gpui-d3rs/src/chart/line.rs` (~120 LOC)
4. `gpui-d3rs/src/chart/bar.rs` (~100 LOC)
5. `gpui-d3rs/examples/chart_demo.rs` (~50 LOC)

**Total: ~450 LOC** (vs ~2000 LOC in original plan)

---

## References

### Internal
- Low-level scatter: `gpui-d3rs/src/shape/scatter.rs`
- Low-level line: `gpui-d3rs/src/shape/line.rs`
- Low-level bar: `gpui-d3rs/src/shape/bar.rs`
- Scales: `gpui-d3rs/src/scale/mod.rs`
- Axis rendering: `gpui-d3rs/src/axis/mod.rs`

### External
- [Plotly Express API](https://plotly.com/python-api-reference/plotly.express.html)
