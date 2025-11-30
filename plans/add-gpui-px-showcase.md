# feat: Add gpui-px Showcase Demo Application

## Overview

Create a comprehensive showcase demo binary for gpui-px that demonstrates all chart types using the Plotly Express-style API. This mirrors the existing `d3rs-showcase` but uses the higher-level gpui-px API.

## Problem Statement / Motivation

- gpui-d3rs has a comprehensive showcase at `bin/showcase.rs` demonstrating all rendering primitives
- gpui-px provides a simpler, Plotly Express-style API but has no demo application
- Users need a visual reference to understand gpui-px capabilities and API usage
- A showcase serves as both documentation and validation that all chart types work correctly

## Proposed Solution

Create `gpui-px/bin/showcase.rs` following the proven sidebar navigation pattern from `gpui-d3rs/bin/showcase.rs`.

### Architecture

```
gpui-px/
├── bin/
│   └── showcase.rs           # Main showcase application (~800-1000 lines)
├── src/
│   ├── lib.rs
│   ├── scatter.rs
│   ├── line.rs
│   ├── bar.rs
│   ├── heatmap.rs
│   ├── contour.rs
│   ├── isoline.rs
│   └── color_scale.rs
└── Cargo.toml                 # Add [[bin]] entry
```

### Sections

| Section | Chart Type | Key Features Demonstrated |
|---------|------------|---------------------------|
| Overview | None | API introduction, code examples |
| Scatter | `scatter()` | Points, colors, sizes |
| Line | `line()` | Time series, trends |
| Bar | `bar()` | Categories, values |
| Heatmap | `heatmap()` | 2D grids, color scales, log scale |
| Contour | `contour()` | Filled bands, thresholds |
| Isoline | `isoline()` | Unfilled lines, levels |
| Gallery | All | 2x3 grid showing all chart types |

## Technical Approach

### 1. Binary Setup

**Cargo.toml addition:**
```toml
[[bin]]
name = "gpui-px-showcase"
path = "bin/showcase.rs"
```

### 2. Application Structure

```rust
use gpui::*;
use gpui_px::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ChartSection {
    #[default]
    Overview,
    Scatter,
    Line,
    Bar,
    Heatmap,
    Contour,
    Isoline,
    Gallery,
}

struct ShowcaseApp {
    current_section: ChartSection,

    // Pre-computed demo data
    scatter_x: Vec<f64>,
    scatter_y: Vec<f64>,
    line_x: Vec<f64>,
    line_y: Vec<f64>,
    bar_categories: Vec<String>,
    bar_values: Vec<f64>,
    heatmap_z: Vec<f64>,
    heatmap_size: usize,
    contour_z: Vec<f64>,
    contour_size: usize,

    // Interactive state
    heatmap_color_scale: ColorScale,
    contour_num_levels: usize,
}
```

### 3. Window Setup

```rust
fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("gpui-px Showcase".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(ShowcaseApp::new),
        )
        .unwrap();
        cx.activate(true);
    });
}
```

### 4. Demo Data Generation

**Scatter:** 100 points in 2 clusters
```rust
fn generate_scatter_data() -> (Vec<f64>, Vec<f64>) {
    let mut x = Vec::new();
    let mut y = Vec::new();

    // Cluster 1: centered at (30, 40)
    for i in 0..50 {
        let angle = (i as f64 * 0.13) * std::f64::consts::PI;
        let r = 8.0 + (i as f64 * 0.1);
        x.push(30.0 + r * angle.cos());
        y.push(40.0 + r * angle.sin());
    }

    // Cluster 2: centered at (70, 60)
    for i in 0..50 {
        let angle = (i as f64 * 0.15) * std::f64::consts::PI;
        let r = 6.0 + (i as f64 * 0.08);
        x.push(70.0 + r * angle.cos());
        y.push(60.0 + r * angle.sin());
    }

    (x, y)
}
```

**Line:** Sine wave with 100 points
```rust
fn generate_line_data() -> (Vec<f64>, Vec<f64>) {
    let x: Vec<f64> = (0..100).map(|i| i as f64 * 0.1).collect();
    let y: Vec<f64> = x.iter().map(|&xi| (xi * 2.0).sin() * 50.0 + 50.0).collect();
    (x, y)
}
```

**Bar:** 7 days of the week
```rust
fn generate_bar_data() -> (Vec<String>, Vec<f64>) {
    let categories = vec!["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        .into_iter().map(String::from).collect();
    let values = vec![45.0, 62.0, 55.0, 78.0, 68.0, 35.0, 28.0];
    (categories, values)
}
```

**Heatmap:** 30x30 Gaussian peaks
```rust
fn generate_heatmap_data(size: usize) -> Vec<f64> {
    let mut z = vec![0.0; size * size];
    for j in 0..size {
        for i in 0..size {
            let x = (i as f64 / size as f64) * 4.0 - 2.0;
            let y = (j as f64 / size as f64) * 4.0 - 2.0;
            // Two Gaussian peaks
            let peak1 = (-((x - 0.5).powi(2) + (y - 0.5).powi(2)) / 0.5).exp();
            let peak2 = 0.7 * (-((x + 0.8).powi(2) + (y + 0.8).powi(2)) / 0.3).exp();
            z[j * size + i] = peak1 + peak2;
        }
    }
    z
}
```

**Contour:** Same as heatmap (50x50)
```rust
fn generate_contour_data(size: usize) -> Vec<f64> {
    generate_heatmap_data(size)  // Reuse heatmap generation
}
```

### 5. Sidebar Navigation

Following the pattern from `gpui-d3rs/bin/showcase.rs:128-176`:

```rust
fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
    div()
        .w(px(200.0))
        .h_full()
        .bg(rgb(0x1e1e1e))
        .flex()
        .flex_col()
        .p_4()
        .gap_1()
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .mb_4()
                .child("gpui-px")
        )
        .children(ChartSection::all().into_iter().map(|section| {
            self.render_sidebar_item(section, cx)
        }))
}

fn render_sidebar_item(&mut self, section: ChartSection, cx: &mut Context<Self>) -> impl IntoElement {
    let is_selected = section == self.current_section;

    div()
        .id(ElementId::Name(section.label().into()))
        .px_3()
        .py_2()
        .rounded_md()
        .cursor_pointer()
        .bg(if is_selected { rgb(0x007acc) } else { rgb(0x1e1e1e) })
        .hover(|s| s.bg(if is_selected { rgb(0x007acc) } else { rgb(0x333333) }))
        .text_sm()
        .text_color(rgb(0xffffff))
        .child(section.label())
        .on_click(cx.listener(move |this, _, _window, _cx| {
            this.current_section = section;
        }))
}
```

### 6. Content Rendering

```rust
fn render_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
    let content = match self.current_section {
        ChartSection::Overview => self.render_overview(),
        ChartSection::Scatter => self.render_scatter_demo(cx),
        ChartSection::Line => self.render_line_demo(),
        ChartSection::Bar => self.render_bar_demo(),
        ChartSection::Heatmap => self.render_heatmap_demo(cx),
        ChartSection::Contour => self.render_contour_demo(cx),
        ChartSection::Isoline => self.render_isoline_demo(),
        ChartSection::Gallery => self.render_gallery(),
    };

    div()
        .id("content-scroll")
        .flex_1()
        .h_full()
        .overflow_y_scroll()
        .bg(rgb(0xffffff))
        .p_8()
        .child(content)
}
```

### 7. Section Demo Pattern

Each section follows this pattern:

```rust
fn render_scatter_demo(&self, cx: &mut Context<Self>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_6()
        // Title
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .child("Scatter Plot")
        )
        // Description
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x666666))
                .max_w(px(600.0))
                .child("Displays individual data points with x,y coordinates. Ideal for exploring correlations and identifying clusters.")
        )
        // Chart
        .child({
            scatter(&self.scatter_x, &self.scatter_y)
                .title("Sample Data")
                .color(0x1f77b4)
                .point_radius(6.0)
                .size(600.0, 400.0)
                .build()
                .unwrap()
        })
        // Code example
        .child(
            div()
                .p_4()
                .bg(rgb(0x2d2d2d))
                .rounded_md()
                .child(
                    div()
                        .text_xs()
                        .font_family("Monaco")
                        .text_color(rgb(0x9cdcfe))
                        .child("scatter(&x, &y)\n    .title(\"My Data\")\n    .color(0x1f77b4)\n    .build()?")
                )
        )
}
```

### 8. Interactive Controls (Heatmap Example)

```rust
fn render_heatmap_demo(&mut self, cx: &mut Context<Self>) -> Div {
    let entity = cx.entity().clone();

    div()
        .flex()
        .flex_col()
        .gap_6()
        .child(/* title and description */)
        // Chart
        .child({
            heatmap(&self.heatmap_z, self.heatmap_size, self.heatmap_size)
                .title("2D Gaussian Peaks")
                .color_scale(self.heatmap_color_scale.clone())
                .size(500.0, 500.0)
                .build()
                .unwrap()
        })
        // Controls
        .child(
            div()
                .flex()
                .flex_col()
                .gap_4()
                .p_4()
                .bg(rgb(0xf8f8f8))
                .rounded_lg()
                .border_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Color Scale")
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .children(vec![
                            ("Viridis", ColorScale::Viridis),
                            ("Plasma", ColorScale::Plasma),
                            ("Inferno", ColorScale::Inferno),
                            ("Heat", ColorScale::Heat),
                        ].into_iter().map(|(label, scale)| {
                            let entity = entity.clone();
                            let is_selected = matches!(
                                (&self.heatmap_color_scale, &scale),
                                (ColorScale::Viridis, ColorScale::Viridis) |
                                (ColorScale::Plasma, ColorScale::Plasma) |
                                (ColorScale::Inferno, ColorScale::Inferno) |
                                (ColorScale::Heat, ColorScale::Heat)
                            );

                            div()
                                .id(ElementId::Name(label.into()))
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .cursor_pointer()
                                .bg(if is_selected { rgb(0x007acc) } else { rgb(0xe0e0e0) })
                                .text_color(if is_selected { rgb(0xffffff) } else { rgb(0x333333) })
                                .text_xs()
                                .child(label)
                                .on_click(move |_, _window, cx| {
                                    let scale = scale.clone();
                                    entity.update(cx, |this, _| {
                                        this.heatmap_color_scale = scale;
                                    });
                                })
                        }))
                )
        )
}
```

### 9. Gallery Section

Display all 6 chart types in a 2x3 grid:

```rust
fn render_gallery(&self) -> Div {
    let small_w = 350.0;
    let small_h = 250.0;

    div()
        .flex()
        .flex_col()
        .gap_6()
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .child("Chart Gallery")
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x666666))
                .child("All gpui-px chart types at a glance")
        )
        // Row 1: Scatter, Line, Bar
        .child(
            div()
                .flex()
                .gap_4()
                .child(
                    scatter(&self.scatter_x, &self.scatter_y)
                        .title("Scatter")
                        .size(small_w, small_h)
                        .build()
                        .unwrap()
                )
                .child(
                    line(&self.line_x, &self.line_y)
                        .title("Line")
                        .size(small_w, small_h)
                        .build()
                        .unwrap()
                )
                .child(
                    bar(&self.bar_categories, &self.bar_values)
                        .title("Bar")
                        .size(small_w, small_h)
                        .build()
                        .unwrap()
                )
        )
        // Row 2: Heatmap, Contour, Isoline
        .child(
            div()
                .flex()
                .gap_4()
                .child(
                    heatmap(&self.heatmap_z, self.heatmap_size, self.heatmap_size)
                        .title("Heatmap")
                        .size(small_w, small_h)
                        .build()
                        .unwrap()
                )
                .child(
                    contour(&self.contour_z, self.contour_size, self.contour_size)
                        .title("Contour")
                        .size(small_w, small_h)
                        .build()
                        .unwrap()
                )
                .child(
                    isoline(&self.contour_z, self.contour_size, self.contour_size)
                        .title("Isoline")
                        .size(small_w, small_h)
                        .build()
                        .unwrap()
                )
        )
}
```

## Acceptance Criteria

### Functional Requirements

- [ ] Binary compiles and runs: `cargo run --bin gpui-px-showcase`
- [ ] Window opens at 1200x800 with "gpui-px Showcase" title
- [ ] Sidebar shows all 8 sections with clickable navigation
- [ ] Active section is visually highlighted
- [ ] Each chart type renders correctly in its section
- [ ] Gallery shows all 6 charts in a 2x3 grid
- [ ] Heatmap color scale selector changes the chart appearance
- [ ] Code examples are displayed in monospace font
- [ ] Content area is scrollable for sections with long content

### Non-Functional Requirements

- [ ] No clippy warnings in the showcase code
- [ ] Renders smoothly (no visible lag on section switching)
- [ ] Window is resizable (minimum 800x600)
- [ ] Code follows existing project patterns from gpui-d3rs showcase

### Quality Gates

- [ ] `cargo check -p gpui-px` passes
- [ ] `cargo clippy -p gpui-px` passes without warnings
- [ ] Showcase runs without panics on all sections

## MVP

### showcase.rs

```rust
//! gpui-px Showcase - Demonstrates all chart types with the Plotly Express-style API.

use gpui::*;
use gpui_px::*;

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("gpui-px Showcase".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(ShowcaseApp::new),
        )
        .unwrap();
        cx.activate(true);
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ChartSection {
    #[default]
    Overview,
    Scatter,
    Line,
    Bar,
    Heatmap,
    Contour,
    Isoline,
    Gallery,
}

impl ChartSection {
    fn all() -> &'static [ChartSection] {
        &[
            ChartSection::Overview,
            ChartSection::Scatter,
            ChartSection::Line,
            ChartSection::Bar,
            ChartSection::Heatmap,
            ChartSection::Contour,
            ChartSection::Isoline,
            ChartSection::Gallery,
        ]
    }

    fn label(&self) -> &'static str {
        match self {
            ChartSection::Overview => "Overview",
            ChartSection::Scatter => "Scatter",
            ChartSection::Line => "Line",
            ChartSection::Bar => "Bar",
            ChartSection::Heatmap => "Heatmap",
            ChartSection::Contour => "Contour",
            ChartSection::Isoline => "Isoline",
            ChartSection::Gallery => "Gallery",
        }
    }
}

struct ShowcaseApp {
    current_section: ChartSection,
    // Demo data
    scatter_x: Vec<f64>,
    scatter_y: Vec<f64>,
    line_x: Vec<f64>,
    line_y: Vec<f64>,
    bar_categories: Vec<String>,
    bar_values: Vec<f64>,
    heatmap_z: Vec<f64>,
    heatmap_size: usize,
    contour_z: Vec<f64>,
    contour_size: usize,
    // Interactive state
    heatmap_color_scale: ColorScale,
}

impl ShowcaseApp {
    fn new(_cx: &mut Context<Self>) -> Self {
        let (scatter_x, scatter_y) = generate_scatter_data();
        let (line_x, line_y) = generate_line_data();
        let (bar_categories, bar_values) = generate_bar_data();
        let heatmap_size = 30;
        let heatmap_z = generate_grid_data(heatmap_size);
        let contour_size = 50;
        let contour_z = generate_grid_data(contour_size);

        Self {
            current_section: ChartSection::default(),
            scatter_x,
            scatter_y,
            line_x,
            line_y,
            bar_categories,
            bar_values,
            heatmap_z,
            heatmap_size,
            contour_z,
            contour_size,
            heatmap_color_scale: ColorScale::Viridis,
        }
    }
}

impl Render for ShowcaseApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .child(self.render_sidebar(cx))
            .child(self.render_content(cx))
    }
}

// Data generators
fn generate_scatter_data() -> (Vec<f64>, Vec<f64>) {
    // Spiral pattern
    (0..100).map(|i| {
        let t = i as f64 * 0.15;
        let r = 10.0 + t * 3.0;
        (50.0 + r * t.cos(), 50.0 + r * t.sin())
    }).unzip()
}

fn generate_line_data() -> (Vec<f64>, Vec<f64>) {
    let x: Vec<f64> = (0..100).map(|i| i as f64 * 0.1).collect();
    let y: Vec<f64> = x.iter().map(|&xi| (xi * 2.0).sin() * 40.0 + 50.0).collect();
    (x, y)
}

fn generate_bar_data() -> (Vec<String>, Vec<f64>) {
    let categories = vec!["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        .into_iter().map(String::from).collect();
    let values = vec![45.0, 62.0, 55.0, 78.0, 68.0, 35.0, 28.0];
    (categories, values)
}

fn generate_grid_data(size: usize) -> Vec<f64> {
    let mut z = vec![0.0; size * size];
    for j in 0..size {
        for i in 0..size {
            let x = (i as f64 / size as f64) * 4.0 - 2.0;
            let y = (j as f64 / size as f64) * 4.0 - 2.0;
            let peak1 = (-((x - 0.5).powi(2) + (y - 0.5).powi(2)) / 0.5).exp();
            let peak2 = 0.7 * (-((x + 0.8).powi(2) + (y + 0.8).powi(2)) / 0.3).exp();
            z[j * size + i] = peak1 + peak2;
        }
    }
    z
}
```

## Implementation Phases

### Phase 1: Binary Setup & Scaffold
- Add `[[bin]]` entry to Cargo.toml
- Create `bin/showcase.rs` with window setup
- Implement `ChartSection` enum and sidebar navigation
- Verify compilation and window display

### Phase 2: Chart Sections
- Implement Overview section with API introduction
- Implement Scatter demo with chart + code example
- Implement Line demo
- Implement Bar demo
- Implement Heatmap demo with color scale selector
- Implement Contour demo
- Implement Isoline demo

### Phase 3: Gallery & Polish
- Implement Gallery section with 2x3 grid
- Add consistent styling across all sections
- Test all sections for rendering correctness
- Run clippy and fix warnings

## References

### Internal References
- gpui-d3rs showcase: `/Users/pierre/src/sotf/gpui-d3rs/bin/showcase.rs`
- gpui-px chart implementations: `/Users/pierre/src/sotf/gpui-px/src/scatter.rs`, `line.rs`, `bar.rs`, `heatmap.rs`, `contour.rs`, `isoline.rs`
- ColorScale enum: `/Users/pierre/src/sotf/gpui-px/src/color_scale.rs`
- GPUI patterns: `/Users/pierre/src/sotf/GPUI.md`

### External References
- GPUI documentation: https://www.gpui.rs/
