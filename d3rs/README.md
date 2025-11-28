# d3rs

A D3.js-inspired plotting library for GPUI with div-based rendering.

## Overview

**d3rs** brings D3.js concepts to Rust and GPUI, providing a powerful yet idiomatic API for data visualization. Unlike D3.js's functional style, d3rs uses Rust's builder patterns and GPUI's component system for a more natural Rust experience.

## Features

### Phase 1 (Completed) ✓

- **Scales**: Linear and logarithmic scales with automatic tick generation
  - `LinearScale` - Map continuous data to visual range
  - `LogScale` - Logarithmic scaling for wide-range data (e.g., frequency)
  - Wilkinson's algorithm for nice tick values

- **Colors**: Rich color system with interpolation
  - `D3Color` - RGB/RGBA color representation
  - Color interpolation for gradients
  - Categorical color schemes (Category10, Tableau10, Pastel)
  - GPUI Rgba conversion

### Phase 2 (Completed) ✓

- **Axes**: Full-featured axis rendering
  - Four orientations: Top, Right, Bottom, Left
  - Custom tick formatters
  - Configurable tick size, padding, and styling
  - Optional domain line
  - Theme integration

- **Grids**: Background grid overlays
  - Dots at tick intersections
  - Horizontal and vertical lines
  - Configurable opacity and styling
  - Multiple preset configurations (dots only, lines only, both)

### Phase 3 (Completed) ✓

- **Shapes**: Basic data visualization marks
  - Bar charts with configurable styling
  - Scatter plots with customizable points
  - Line charts with multiple curve types (Linear, Step, StepBefore, StepAfter)
  - Support for negative values in bar charts
  - Multiple series rendering
  - Optional strokes and fills

### Upcoming

- **Phase 4**: Areas, legends, and advanced features

## Quick Start

```rust
use d3rs::prelude::*;

// Create a linear scale
let scale = LinearScale::new()
    .domain(0.0, 100.0)
    .range(0.0, 500.0);

assert_eq!(scale.scale(50.0), 250.0);

// Create a log scale for frequencies
let freq_scale = LogScale::new()
    .domain(20.0, 20000.0)  // 20Hz - 20kHz
    .range(0.0, 1.0);       // Normalized coordinates

// Generate nice tick values
let ticks = scale.ticks(10);

// Color schemes
let scheme = ColorScheme::category10();
let color0 = scheme.color(0);  // Blue
let color1 = scheme.color(1);  // Orange
```

## Examples

Run the demonstrations:

```bash
# Scales and colors
cargo run -p d3rs --example scale_demo

# Axes in all orientations
cargo run -p d3rs --example axis_demo

# Grid configurations
cargo run -p d3rs --example grid_demo

# Bar charts
cargo run -p d3rs --example bar_chart_demo

# Scatter plots
cargo run -p d3rs --example scatter_demo

# Line charts with different curve types
cargo run -p d3rs --example line_chart_demo
```

## Testing

Run the test suite:

```bash
cargo check -p d3rs
cargo check -p d3rs --examples
```

Phase 1 tests: **38/38** ✓
Phase 2: Verified via examples (GPUI rendering requires runtime)

## Architecture

d3rs is designed with the following principles:

1. **Builder Pattern API**: Idiomatic Rust, not D3's functional chaining
2. **Div-Based Rendering**: All shapes built with GPUI divs using `relative()` and `px()`
3. **Scale-Driven**: Scales are the foundation of all visualizations
4. **GPUI Integration**: Native integration with GPUI's theming and component system

## API Comparison: D3.js vs d3rs

### Scales

**D3.js**:
```javascript
const scale = d3.scaleLinear()
  .domain([0, 100])
  .range([0, 500]);

const value = scale(50); // 250
```

**d3rs**:
```rust
let scale = LinearScale::new()
    .domain(0.0, 100.0)
    .range(0.0, 500.0);

let value = scale.scale(50.0); // 250.0
```

### Color Schemes

**D3.js**:
```javascript
const color = d3.scaleOrdinal(d3.schemeCategory10);
```

**d3rs**:
```rust
let scheme = ColorScheme::category10();
let color = scheme.color(0);
```

## Roadmap

- [x] **Phase 1**: Core infrastructure (scales, colors)
- [x] **Phase 2**: Axes and grids
- [x] **Phase 3**: Basic shapes (bars, lines, scatter)
- [ ] **Phase 4**: Advanced features (areas, legends, documentation)

## License

MIT OR Apache-2.0
