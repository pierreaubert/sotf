# feat: Add contour, heatmap, and isoline support to gpui-px

## Overview

Add high-level Plotly Express-style APIs for 2D grid visualization to gpui-px:
- **Heatmap**: Colored grid cells for visualizing matrices
- **Contour**: Filled bands between threshold levels
- **Isoline**: Line contours at specific threshold values (separate function, not a boolean flag)

These wrap existing gpui-d3rs primitives (`render_heatmap`, `render_contour`, `render_contour_bands`) with ergonomic builder APIs matching the existing `scatter()`, `line()`, and `bar()` patterns.

## Problem Statement / Motivation

**Why this matters:**

1. **Audio visualization**: Spectrograms require heatmaps with log-frequency axes
2. **Scientific data**: Contour plots are essential for 2D field visualization
3. **API completeness**: gpui-px currently lacks 2D grid visualization, limiting its usefulness
4. **Consistency**: gpui-d3rs has the primitives, but they require verbose setup

**Current state:**
```rust
// gpui-d3rs (verbose, low-level)
let generator = ContourGenerator::new(width, height)
    .x_values(x_coords.clone())
    .y_values(y_coords.clone());
let contours = generator.contours(&z_values, &thresholds);
let x_scale = LogScale::new().domain(20.0, 20000.0).range(0.0, 400.0);
let y_scale = LinearScale::new().domain(-20.0, 10.0).range(0.0, 300.0);
let config = ContourConfig::new().fill(true).color_scale(viridis_color_scale());
let element = render_contour(&contours, &x_scale, &y_scale, &config);
```

**Target state (gpui-px):**
```rust
// gpui-px (ergonomic, high-level)
let chart = contour(&z, 100, 50)
    .x(&freq_bins)
    .y(&time_bins)
    .x_scale(ScaleType::Log)
    .thresholds(vec![-15.0, -10.0, -5.0, 0.0, 5.0])
    .color_scale(ColorScale::Viridis)
    .build()?;
```

## Proposed Solution

### API Design

#### 1. Heatmap API

```rust
// Basic heatmap with flat array (consistent with scatter/line/bar patterns)
let chart = heatmap(&z, width, height)
    .title("Temperature Grid")
    .build()?;

// With explicit coordinates and log scale
let chart = heatmap(&z, 100, 50)
    .x(&freq_bins)           // 100 values
    .y(&time_bins)           // 50 values
    .title("Spectrogram")
    .x_scale(ScaleType::Log)
    .color_scale(ColorScale::Inferno)
    .color_range(-80.0, 0.0)
    .build()?;
```

#### 2. Contour API (filled bands)

```rust
// Auto-generated thresholds
let chart = contour(&z, width, height)
    .x(&x_coords)
    .y(&y_coords)
    .threshold_count(10)
    .color_scale(ColorScale::Viridis)
    .build()?;

// Explicit thresholds
let chart = contour(&z, width, height)
    .thresholds(vec![0.0, 25.0, 50.0, 75.0, 100.0])
    .build()?;
```

#### 3. Isoline API (separate function, not a boolean flag)

```rust
// Isolines at specific levels - separate function for clarity
let chart = isoline(&z, width, height)
    .x(&x_coords)
    .y(&y_coords)
    .levels(vec![0.0, 50.0, 100.0])
    .stroke_width(2.0)
    .color(0x333333)  // Single color for all lines
    .build()?;
```

### File Structure

```
gpui-px/src/
├── lib.rs           # Add exports, grid validation utilities, ScaleType enum
├── error.rs         # Add GridDimensionMismatch error variant
├── heatmap.rs       # NEW: HeatmapChart builder
├── contour.rs       # NEW: ContourChart builder
├── isoline.rs       # NEW: IsolineChart builder
└── color_scale.rs   # NEW: ColorScale enum with preset colormaps
```

## Technical Approach

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      gpui-px (High-Level API)               │
├─────────────────────────────────────────────────────────────┤
│  heatmap(&z, w, h)    contour(&z, w, h)    isoline(&z, w, h)│
│    .x(&x)               .x(&x)               .x(&x)         │
│    .y(&y)               .y(&y)               .y(&y)         │
│    .x_scale(Log)        .thresholds()        .levels()      │
│    .color_scale()       .color_scale()       .color()       │
│    .build()             .build()             .build()       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    gpui-d3rs (Low-Level)                    │
├─────────────────────────────────────────────────────────────┤
│  ContourGenerator        HeatmapData                        │
│    .x_values()            .new(x, y, values)                │
│    .y_values()                                              │
│    .contours()           render_heatmap()                   │
│    .contour_bands()      render_contour()                   │
│                          render_contour_bands()             │
├─────────────────────────────────────────────────────────────┤
│  LinearScale             LogScale           ContourConfig   │
│    .domain()              .domain()          .color_scale() │
│    .range()               .range()           .fill()        │
└─────────────────────────────────────────────────────────────┘
```

### Implementation Tasks

#### 1. Error Handling (error.rs)

Add one new error variant (reuse `InvalidData` for other validation errors):

```rust
// gpui-px/src/error.rs
#[derive(Debug, Error)]
pub enum ChartError {
    // ... existing variants ...

    #[error("Grid dimension mismatch: z has {z_len} values but expected {width} x {height} = {expected}")]
    GridDimensionMismatch {
        z_len: usize,
        width: usize,
        height: usize,
        expected: usize,
    },
}

// Use existing InvalidData for:
// - "log scale requires positive values"
// - "coordinates must be monotonically increasing"
// - "grid too small (need at least 2x2)"
```

#### 2. Grid Validation Utilities (lib.rs)

```rust
// gpui-px/src/lib.rs

/// Validate grid dimensions match z array length.
pub(crate) fn validate_grid_dimensions(
    z_len: usize,
    width: usize,
    height: usize,
) -> Result<(), ChartError> {
    let expected = width * height;
    if z_len != expected {
        return Err(ChartError::GridDimensionMismatch {
            z_len,
            width,
            height,
            expected,
        });
    }
    if width < 2 || height < 2 {
        return Err(ChartError::InvalidData {
            field: "grid",
            reason: "need at least 2x2 grid for contour generation",
        });
    }
    Ok(())
}

/// Validate coordinates are monotonically increasing.
pub(crate) fn validate_monotonic(
    values: &[f64],
    field: &'static str,
) -> Result<(), ChartError> {
    if !values.windows(2).all(|w| w[0] < w[1]) {
        return Err(ChartError::InvalidData {
            field,
            reason: "coordinates must be monotonically increasing",
        });
    }
    Ok(())
}

/// Validate all values are positive (for log scale).
pub(crate) fn validate_positive(
    values: &[f64],
    field: &'static str,
) -> Result<(), ChartError> {
    if values.iter().any(|&v| v <= 0.0) {
        return Err(ChartError::InvalidData {
            field,
            reason: "log scale requires positive values",
        });
    }
    Ok(())
}

/// Scale type for axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScaleType {
    #[default]
    Linear,
    Log,
}
```

#### 3. Color Scale Module (color_scale.rs)

```rust
// gpui-px/src/color_scale.rs
use d3rs::color::D3Color;
use std::sync::Arc;

/// Predefined color scales for heatmaps and contours.
#[derive(Clone)]
pub enum ColorScale {
    /// Perceptually uniform, colorblind-friendly (blue → green → yellow)
    Viridis,
    /// Perceptually uniform (blue → purple → orange → yellow)
    Plasma,
    /// Perceptually uniform (black → purple → orange → yellow)
    Inferno,
    /// Perceptually uniform (black → purple → pink → white)
    Magma,
    /// Diverging (blue → white → red) - good for values with meaningful zero
    Heat,
    /// Diverging (cool blue → warm red)
    Coolwarm,
    /// Sequential grayscale (black → white)
    Greys,
    /// Custom color function: takes normalized value [0, 1] → color
    Custom(Arc<dyn Fn(f64) -> D3Color + Send + Sync>),
}

impl Default for ColorScale {
    fn default() -> Self {
        ColorScale::Viridis
    }
}

impl ColorScale {
    /// Create a custom color scale from a function.
    pub fn custom<F>(f: F) -> Self
    where
        F: Fn(f64) -> D3Color + Send + Sync + 'static,
    {
        ColorScale::Custom(Arc::new(f))
    }

    /// Convert to the color function used by gpui-d3rs.
    pub(crate) fn to_color_fn(&self) -> Arc<dyn Fn(f64) -> D3Color + Send + Sync> {
        match self {
            ColorScale::Viridis => Arc::new(viridis),
            ColorScale::Plasma => Arc::new(plasma),
            ColorScale::Inferno => Arc::new(inferno),
            ColorScale::Magma => Arc::new(magma),
            ColorScale::Heat => Arc::new(heat),
            ColorScale::Coolwarm => Arc::new(coolwarm),
            ColorScale::Greys => Arc::new(greys),
            ColorScale::Custom(f) => f.clone(),
        }
    }
}

// Color scale implementations (interpolate t in [0, 1])
fn viridis(t: f64) -> D3Color { /* ... */ }
fn plasma(t: f64) -> D3Color { /* ... */ }
fn inferno(t: f64) -> D3Color { /* ... */ }
fn magma(t: f64) -> D3Color { /* ... */ }
fn heat(t: f64) -> D3Color { /* ... */ }
fn coolwarm(t: f64) -> D3Color { /* ... */ }
fn greys(t: f64) -> D3Color { /* ... */ }

impl std::fmt::Debug for ColorScale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColorScale::Viridis => write!(f, "Viridis"),
            ColorScale::Plasma => write!(f, "Plasma"),
            ColorScale::Inferno => write!(f, "Inferno"),
            ColorScale::Magma => write!(f, "Magma"),
            ColorScale::Heat => write!(f, "Heat"),
            ColorScale::Coolwarm => write!(f, "Coolwarm"),
            ColorScale::Greys => write!(f, "Greys"),
            ColorScale::Custom(_) => write!(f, "Custom"),
        }
    }
}
```

#### 4. Heatmap Implementation (heatmap.rs)

```rust
// gpui-px/src/heatmap.rs
use crate::{
    ColorScale, ScaleType, ChartError,
    validate_grid_dimensions, validate_monotonic, validate_positive,
    validate_data_array, validate_dimensions,
    DEFAULT_WIDTH, DEFAULT_HEIGHT, TITLE_AREA_HEIGHT, DEFAULT_TITLE_FONT_SIZE,
};

/// Heatmap chart builder.
#[derive(Debug, Clone)]
pub struct HeatmapChart {
    z: Vec<f64>,
    width: usize,
    height: usize,
    x: Option<Vec<f64>>,
    y: Option<Vec<f64>>,
    title: Option<String>,
    color_scale: ColorScale,
    color_range: Option<(f64, f64)>,
    x_scale_type: ScaleType,
    y_scale_type: ScaleType,
    opacity: f32,
    chart_width: f32,
    chart_height: f32,
}

impl HeatmapChart {
    /// Set explicit x coordinates (must have `width` values).
    pub fn x(mut self, coords: &[f64]) -> Self {
        self.x = Some(coords.to_vec());
        self
    }

    /// Set explicit y coordinates (must have `height` values).
    pub fn y(mut self, coords: &[f64]) -> Self {
        self.y = Some(coords.to_vec());
        self
    }

    /// Set chart title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set color scale (default: Viridis).
    pub fn color_scale(mut self, scale: ColorScale) -> Self {
        self.color_scale = scale;
        self
    }

    /// Set explicit color range. If not set, auto-computed from z values.
    pub fn color_range(mut self, min: f64, max: f64) -> Self {
        self.color_range = Some((min, max));
        self
    }

    /// Set x-axis scale type (default: Linear).
    pub fn x_scale(mut self, scale: ScaleType) -> Self {
        self.x_scale_type = scale;
        self
    }

    /// Set y-axis scale type (default: Linear).
    pub fn y_scale(mut self, scale: ScaleType) -> Self {
        self.y_scale_type = scale;
        self
    }

    /// Set opacity (0.0 - 1.0).
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Set chart dimensions.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.chart_width = width;
        self.chart_height = height;
        self
    }

    /// Build and validate the chart.
    pub fn build(self) -> Result<impl IntoElement, ChartError> {
        // Validate grid dimensions
        validate_grid_dimensions(self.z.len(), self.width, self.height)?;
        validate_dimensions(self.chart_width, self.chart_height)?;

        // Generate default coordinates if not provided
        let x_coords = self.x.unwrap_or_else(|| {
            (0..self.width).map(|i| i as f64).collect()
        });
        let y_coords = self.y.unwrap_or_else(|| {
            (0..self.height).map(|i| i as f64).collect()
        });

        // Validate coordinate lengths
        if x_coords.len() != self.width {
            return Err(ChartError::DataLengthMismatch {
                x_field: "x",
                y_field: "width",
                x_len: x_coords.len(),
                y_len: self.width,
            });
        }
        if y_coords.len() != self.height {
            return Err(ChartError::DataLengthMismatch {
                x_field: "y",
                y_field: "height",
                x_len: y_coords.len(),
                y_len: self.height,
            });
        }

        // Validate coordinates are monotonic
        validate_monotonic(&x_coords, "x")?;
        validate_monotonic(&y_coords, "y")?;

        // Validate log scale requirements
        if self.x_scale_type == ScaleType::Log {
            validate_positive(&x_coords, "x")?;
        }
        if self.y_scale_type == ScaleType::Log {
            validate_positive(&y_coords, "y")?;
        }

        // Calculate plot area
        let title_height = if self.title.is_some() { TITLE_AREA_HEIGHT } else { 0.0 };
        let plot_height = self.chart_height - title_height;

        // Calculate color range
        let (z_min, z_max) = self.color_range.unwrap_or_else(|| {
            let min = self.z.iter().cloned().filter(|v| v.is_finite()).fold(f64::INFINITY, f64::min);
            let max = self.z.iter().cloned().filter(|v| v.is_finite()).fold(f64::NEG_INFINITY, f64::max);
            (min, max)
        });

        // Create scales using gpui-d3rs (use x_values/y_values for non-linear grids)
        let x_domain = (x_coords[0], x_coords[x_coords.len() - 1]);
        let y_domain = (y_coords[0], y_coords[y_coords.len() - 1]);

        // Build HeatmapData and render using gpui-d3rs
        // ... (implementation calls render_heatmap with appropriate scales)

        // Wrap in container with title (same pattern as scatter/line/bar)
        let mut container = div()
            .w(px(self.chart_width))
            .h(px(self.chart_height))
            .relative()
            .flex()
            .flex_col();

        // Add title if present
        if let Some(title) = &self.title {
            let font_config = VectorFontConfig::horizontal(
                DEFAULT_TITLE_FONT_SIZE,
                hsla(0.0, 0.0, 0.2, 1.0),
            );
            container = container.child(
                div()
                    .w_full()
                    .h(px(title_height))
                    .flex()
                    .justify_center()
                    .items_center()
                    .child(render_vector_text(title, &font_config)),
            );
        }

        // Add plot area with heatmap element
        // container = container.child(heatmap_element);

        Ok(container)
    }
}

/// Create a heatmap from flattened row-major grid data.
///
/// # Arguments
/// * `z` - Flattened row-major grid values: `z[row * width + col]`
/// * `width` - Number of columns
/// * `height` - Number of rows
///
/// # Example
/// ```rust,no_run
/// use gpui_px::{heatmap, ColorScale, ScaleType};
///
/// // Simple 3x2 heatmap
/// let z = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
/// let chart = heatmap(&z, 3, 2)
///     .title("My Heatmap")
///     .color_scale(ColorScale::Viridis)
///     .build()?;
///
/// // Spectrogram with log frequency axis
/// let chart = heatmap(&spectrogram_data, 100, 50)
///     .x(&freq_bins)
///     .y(&time_bins)
///     .x_scale(ScaleType::Log)
///     .color_scale(ColorScale::Inferno)
///     .color_range(-80.0, 0.0)
///     .build()?;
/// # Ok::<(), gpui_px::ChartError>(())
/// ```
pub fn heatmap(z: &[f64], width: usize, height: usize) -> HeatmapChart {
    HeatmapChart {
        z: z.to_vec(),
        width,
        height,
        x: None,
        y: None,
        title: None,
        color_scale: ColorScale::default(),
        color_range: None,
        x_scale_type: ScaleType::default(),
        y_scale_type: ScaleType::default(),
        opacity: 1.0,
        chart_width: DEFAULT_WIDTH,
        chart_height: DEFAULT_HEIGHT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heatmap_empty_data() {
        let result = heatmap(&[], 0, 0).build();
        assert!(matches!(result, Err(ChartError::InvalidData { .. })));
    }

    #[test]
    fn test_heatmap_dimension_mismatch() {
        let z = vec![1.0, 2.0, 3.0, 4.0];
        let result = heatmap(&z, 3, 2).build(); // 3*2=6 != 4
        assert!(matches!(result, Err(ChartError::GridDimensionMismatch { .. })));
    }

    #[test]
    fn test_heatmap_x_length_mismatch() {
        let z = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let x = vec![0.0, 1.0]; // Wrong: need 3 values
        let result = heatmap(&z, 3, 2).x(&x).build();
        assert!(matches!(result, Err(ChartError::DataLengthMismatch { .. })));
    }

    #[test]
    fn test_heatmap_log_scale_negative_values() {
        let z = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let x = vec![-1.0, 0.0, 1.0]; // Negative values
        let result = heatmap(&z, 3, 2).x(&x).x_scale(ScaleType::Log).build();
        assert!(matches!(result, Err(ChartError::InvalidData { .. })));
    }

    #[test]
    fn test_heatmap_successful_build() {
        let z = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let result = heatmap(&z, 3, 2)
            .title("Test Heatmap")
            .color_scale(ColorScale::Viridis)
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_heatmap_with_log_scale() {
        let z = vec![1.0; 100];
        let x: Vec<f64> = (0..10).map(|i| 20.0 * (1000.0_f64).powf(i as f64 / 9.0)).collect();
        let y: Vec<f64> = (0..10).map(|i| i as f64 + 1.0).collect();
        let result = heatmap(&z, 10, 10)
            .x(&x)
            .y(&y)
            .x_scale(ScaleType::Log)
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_heatmap_opacity_clamp() {
        let z = vec![1.0, 2.0, 3.0, 4.0];
        let chart = heatmap(&z, 2, 2).opacity(1.5);
        assert_eq!(chart.opacity, 1.0);

        let chart = heatmap(&z, 2, 2).opacity(-0.5);
        assert_eq!(chart.opacity, 0.0);
    }
}
```

#### 5. Contour Implementation (contour.rs)

```rust
// gpui-px/src/contour.rs
// Similar structure to heatmap.rs but with threshold support

/// Contour chart builder (filled bands).
#[derive(Debug, Clone)]
pub struct ContourChart {
    z: Vec<f64>,
    width: usize,
    height: usize,
    x: Option<Vec<f64>>,
    y: Option<Vec<f64>>,
    title: Option<String>,
    thresholds: Option<Vec<f64>>,
    threshold_count: usize,
    color_scale: ColorScale,
    x_scale_type: ScaleType,
    y_scale_type: ScaleType,
    opacity: f32,
    chart_width: f32,
    chart_height: f32,
}

impl ContourChart {
    // Builder methods similar to HeatmapChart...

    /// Set explicit threshold levels.
    pub fn thresholds(mut self, levels: Vec<f64>) -> Self {
        self.thresholds = Some(levels);
        self
    }

    /// Set number of auto-generated thresholds (default: 10).
    pub fn threshold_count(mut self, count: usize) -> Self {
        self.threshold_count = count;
        self
    }

    pub fn build(self) -> Result<impl IntoElement, ChartError> {
        // Validation...

        // Generate thresholds if not provided
        let thresholds = self.thresholds.unwrap_or_else(|| {
            generate_thresholds(&self.z, self.threshold_count)
        });

        // Use ContourGenerator with x_values/y_values (NOT x/y with min/max)
        let generator = ContourGenerator::new(self.width, self.height)
            .x_values(x_coords.clone())
            .y_values(y_coords.clone());

        let contour_bands = generator.contour_bands(&self.z, &thresholds);

        // Render using render_contour_bands...
        Ok(container)
    }
}

fn generate_thresholds(z: &[f64], count: usize) -> Vec<f64> {
    let min = z.iter().cloned().filter(|v| v.is_finite()).fold(f64::INFINITY, f64::min);
    let max = z.iter().cloned().filter(|v| v.is_finite()).fold(f64::NEG_INFINITY, f64::max);
    (0..=count)
        .map(|i| min + (max - min) * (i as f64 / count as f64))
        .collect()
}

/// Create a filled contour chart from flattened row-major grid data.
pub fn contour(z: &[f64], width: usize, height: usize) -> ContourChart {
    ContourChart {
        z: z.to_vec(),
        width,
        height,
        x: None,
        y: None,
        title: None,
        thresholds: None,
        threshold_count: 10,
        color_scale: ColorScale::default(),
        x_scale_type: ScaleType::default(),
        y_scale_type: ScaleType::default(),
        opacity: 1.0,
        chart_width: DEFAULT_WIDTH,
        chart_height: DEFAULT_HEIGHT,
    }
}
```

#### 6. Isoline Implementation (isoline.rs)

```rust
// gpui-px/src/isoline.rs
// Separate from contour - clearer API, no boolean flag

/// Isoline chart builder (line contours at specific levels).
#[derive(Debug, Clone)]
pub struct IsolineChart {
    z: Vec<f64>,
    width: usize,
    height: usize,
    x: Option<Vec<f64>>,
    y: Option<Vec<f64>>,
    title: Option<String>,
    levels: Vec<f64>,
    color: u32,              // Single color for all lines (consistent with line chart)
    stroke_width: f32,
    opacity: f32,
    x_scale_type: ScaleType,
    y_scale_type: ScaleType,
    chart_width: f32,
    chart_height: f32,
}

impl IsolineChart {
    /// Set isoline levels (required).
    pub fn levels(mut self, levels: Vec<f64>) -> Self {
        self.levels = levels;
        self
    }

    /// Set line color as 24-bit RGB hex (format: 0xRRGGBB).
    pub fn color(mut self, hex: u32) -> Self {
        self.color = hex;
        self
    }

    /// Set line stroke width.
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width;
        self
    }

    /// Set opacity (0.0 - 1.0).
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn build(self) -> Result<impl IntoElement, ChartError> {
        if self.levels.is_empty() {
            return Err(ChartError::EmptyData { field: "levels" });
        }

        // Use ContourGenerator.contours() (not contour_bands)
        let contours = generator.contours(&self.z, &self.levels);

        // Render using render_contour with fill=false
        Ok(container)
    }
}

/// Create an isoline chart (line contours) from flattened row-major grid data.
///
/// # Example
/// ```rust,no_run
/// use gpui_px::isoline;
///
/// let z = compute_elevation_grid();
/// let chart = isoline(&z, 100, 100)
///     .x(&lon_coords)
///     .y(&lat_coords)
///     .levels(vec![0.0, 100.0, 200.0, 500.0, 1000.0])
///     .color(0x333333)
///     .stroke_width(1.5)
///     .build()?;
/// # Ok::<(), gpui_px::ChartError>(())
/// ```
pub fn isoline(z: &[f64], width: usize, height: usize) -> IsolineChart {
    IsolineChart {
        z: z.to_vec(),
        width,
        height,
        x: None,
        y: None,
        title: None,
        levels: vec![],
        color: 0x333333,
        stroke_width: 1.5,
        opacity: 1.0,
        x_scale_type: ScaleType::default(),
        y_scale_type: ScaleType::default(),
        chart_width: DEFAULT_WIDTH,
        chart_height: DEFAULT_HEIGHT,
    }
}
```

#### 7. Update lib.rs Exports

```rust
// gpui-px/src/lib.rs
mod color_scale;
mod contour;
mod heatmap;
mod isoline;

pub use color_scale::ColorScale;
pub use contour::{contour, ContourChart};
pub use heatmap::{heatmap, HeatmapChart};
pub use isoline::{isoline, IsolineChart};

// ScaleType defined in lib.rs (see above)
pub use crate::ScaleType;
```

## Acceptance Criteria

### Functional Requirements

- [ ] `heatmap(&z, w, h)` creates a heatmap with implicit 0-based coordinates
- [ ] `heatmap(&z, w, h).x(&x).y(&y)` works with explicit coordinates
- [ ] `heatmap(...).x_scale(ScaleType::Log)` works for log-frequency data
- [ ] `heatmap(...).color_scale(ColorScale::Viridis)` applies color mapping
- [ ] `heatmap(...).color_range(min, max)` clamps color mapping
- [ ] `contour(&z, w, h).threshold_count(10)` generates evenly-spaced thresholds
- [ ] `contour(&z, w, h).thresholds(vec![...])` uses explicit thresholds
- [ ] `isoline(&z, w, h).levels(vec![...])` renders line contours
- [ ] All charts support `.title()`, `.size()`, `.opacity()`, `.build()`
- [ ] All color scales work: Viridis, Plasma, Inferno, Magma, Heat, Coolwarm, Greys, Custom

### Non-Functional Requirements

- [ ] Grid validation catches dimension mismatches before rendering
- [ ] NaN values in z are skipped (rendered as gaps)
- [ ] Log scale validates positive coordinate values
- [ ] Opacity methods clamp values to [0.0, 1.0]

### Quality Gates

- [ ] Unit tests for all validation cases
- [ ] Unit tests for builder methods
- [ ] Documentation for all public APIs
- [ ] `cargo check -p gpui-px` passes
- [ ] `cargo clippy -p gpui-px` has no warnings
- [ ] Test names follow existing pattern: `test_{chart}_successful_build`

## Dependencies

- **gpui-d3rs**: Already has `ContourGenerator`, `render_heatmap`, `render_contour`, `render_contour_bands`
- **gpui**: Already a dependency
- **No new external dependencies required**

## Review Feedback Incorporated

1. ✅ **Flat array API** - Factory functions now use `fn(z: &[f64], width, height)` matching existing patterns
2. ✅ **Separate isoline function** - No boolean flag; `isoline()` is a separate function with its own builder
3. ✅ **Opacity clamping** - All opacity setters use `.clamp(0.0, 1.0)`
4. ✅ **Reuse InvalidData** - Only one new error variant (`GridDimensionMismatch`); others use existing `InvalidData`
5. ✅ **Correct coordinate handling** - Uses `x_values()`/`y_values()` not `x(min, max)`
6. ✅ **Test naming** - Uses `test_{chart}_successful_build` pattern
7. ✅ **Keep full scope** - Log scale and all 7 color scales preserved as requested
