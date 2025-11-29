//! d3rs Showcase - Unified demo application
//!
//! Demonstrates all d3rs functionality in a single application with tabbed navigation.

use d3rs::axis::{render_axis, AxisConfig, DefaultAxisTheme};
use d3rs::color::{ColorScheme, D3Color};
use d3rs::contour::{ContourGenerator, DensityEstimator};
use d3rs::grid::{render_grid, GridConfig};
use d3rs::prelude::*;
use d3rs::shape::contour::{render_contour, ContourConfig, viridis_color_scale, heat_color_scale};
use gpui::{Menu, MenuItem, *};
use gpui_ui_kit::Slider;

// Demo sections
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum DemoSection {
    #[default]
    Overview,
    Scales,
    Axes,
    BarCharts,
    LineCharts,
    ScatterPlots,
    Contours,
    Colors,
}

impl DemoSection {
    fn all() -> Vec<Self> {
        vec![
            Self::Overview,
            Self::Scales,
            Self::Axes,
            Self::BarCharts,
            Self::LineCharts,
            Self::ScatterPlots,
            Self::Contours,
            Self::Colors,
        ]
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Scales => "Scales",
            Self::Axes => "Axes",
            Self::BarCharts => "Bar Charts",
            Self::LineCharts => "Line Charts",
            Self::ScatterPlots => "Scatter Plots",
            Self::Contours => "Contours",
            Self::Colors => "Colors",
        }
    }
}

struct ShowcaseApp {
    current_section: DemoSection,
    // Contour demo parameters
    contour_grid_size: usize,
    contour_num_levels: usize,
    contour_peak1_x: f32,
    contour_peak1_y: f32,
    contour_peak2_x: f32,
    contour_peak2_y: f32,
    density_bandwidth: f32,
    density_num_points: usize,
}

impl ShowcaseApp {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            current_section: DemoSection::default(),
            contour_grid_size: 50,
            contour_num_levels: 5,
            contour_peak1_x: 0.3,
            contour_peak1_y: 0.3,
            contour_peak2_x: -0.4,
            contour_peak2_y: -0.2,
            density_bandwidth: 0.08,
            density_num_points: 100,
        }
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.current_section;

        div()
            .w(px(200.0))
            .h_full()
            .bg(rgb(0x1e1e1e))
            .border_r_1()
            .border_color(rgb(0x3c3c3c))
            .flex()
            .flex_col()
            .p_4()
            .gap_2()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .mb_4()
                    .child("d3rs Showcase"),
            )
            .children(DemoSection::all().into_iter().map(|section| {
                let is_selected = section == current;
                let bg = if is_selected {
                    rgb(0x007acc)
                } else {
                    rgb(0x1e1e1e)
                };
                let hover_bg = if is_selected {
                    rgb(0x007acc)
                } else {
                    rgb(0x2d2d2d)
                };

                div()
                    .id(ElementId::Name(section.label().into()))
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(bg)
                    .hover(|s| s.bg(hover_bg))
                    .text_color(rgb(0xffffff))
                    .child(section.label())
                    .on_click(cx.listener(move |this, _, _window, _cx| {
                        this.current_section = section;
                    }))
            }))
    }

    fn render_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let content: Div = match self.current_section {
            DemoSection::Overview => self.render_overview(),
            DemoSection::Scales => self.render_scales_demo(),
            DemoSection::Axes => self.render_axes_demo(),
            DemoSection::BarCharts => self.render_bar_chart_demo(),
            DemoSection::LineCharts => self.render_line_chart_demo(),
            DemoSection::ScatterPlots => self.render_scatter_demo(),
            DemoSection::Contours => self.render_contours_demo(cx),
            DemoSection::Colors => self.render_colors_demo(),
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

    fn render_overview(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .child("Welcome to d3rs"),
            )
            .child(
                div()
                    .text_base()
                    .text_color(rgb(0x666666))
                    .max_w(px(600.0))
                    .child("d3rs is a D3.js-inspired plotting library for GPUI. It brings familiar D3 concepts like scales, axes, and shape generators to Rust applications built with GPUI."),
            )
            .child(
                div()
                    .mt_4()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Features"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .ml_4()
                    .child(self.feature_item("Scales", "Linear, logarithmic, ordinal, and band scales"))
                    .child(self.feature_item("Axes", "Customizable axes with tick formatting"))
                    .child(self.feature_item("Charts", "Bar charts, line charts, scatter plots"))
                    .child(self.feature_item("Colors", "Color schemes and interpolation"))
                    .child(self.feature_item("Shapes", "Arcs, pies, symbols, curves, and more"))
                    .child(self.feature_item("Data", "Statistics, binning, and transformations")),
            )
            .child(
                div()
                    .mt_6()
                    .p_4()
                    .bg(rgb(0xf5f5f5))
                    .rounded_md()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child("Use the sidebar to explore different demos"),
                    ),
            )
    }

    fn feature_item(&self, title: &str, desc: &str) -> Div {
        div()
            .flex()
            .gap_2()
            .child(
                div()
                    .text_color(rgb(0x007acc))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("{title}:")),
            )
            .child(div().text_color(rgb(0x666666)).child(desc.to_string()))
    }

    fn render_scales_demo(&self) -> Div {
        let linear = LinearScale::new().domain(0.0, 100.0).range(0.0, 500.0);
        let log_scale = LogScale::new().domain(20.0, 20000.0).range(0.0, 1.0);

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .child("Scales Demo"),
            )
            // Linear scale
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Linear Scale (0-100 -> 0-500)"),
                    )
                    .child(self.scale_table(
                        &[0.0, 25.0, 50.0, 75.0, 100.0],
                        |v| format!("{:.0}", linear.scale(v)),
                    )),
            )
            // Log scale
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Logarithmic Scale (20Hz-20kHz -> 0-1)"),
                    )
                    .child(self.scale_table(
                        &[20.0, 100.0, 1000.0, 10000.0, 20000.0],
                        |v| format!("{:.3}", log_scale.scale(v)),
                    )),
            )
            // Ticks
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Generated Ticks"),
                    )
                    .child(
                        div()
                            .p_3()
                            .bg(rgb(0xf5f5f5))
                            .rounded_md()
                            .child(
                                div()
                                    .text_sm()
                                    .child(format!("Linear ticks: {:?}", linear.ticks(5))),
                            ),
                    ),
            )
    }

    fn scale_table<F>(&self, values: &[f64], transform: F) -> Div
    where
        F: Fn(f64) -> String,
    {
        div()
            .p_3()
            .bg(rgb(0xf5f5f5))
            .rounded_md()
            .flex()
            .flex_col()
            .gap_1()
            .children(values.iter().map(|v| {
                div()
                    .flex()
                    .gap_4()
                    .text_sm()
                    .child(div().w(px(80.0)).child(format!("{:.0}", v)))
                    .child(div().text_color(rgb(0x666666)).child("->"))
                    .child(div().font_weight(FontWeight::MEDIUM).child(transform(*v)))
            }))
    }

    fn render_axes_demo(&self) -> Div {
        let theme = DefaultAxisTheme;
        let x_scale = LinearScale::new().domain(0.0, 100.0).range(0.0, 400.0);
        let freq_scale = LogScale::new().domain(20.0, 20000.0).range(0.0, 400.0);
        let db_scale = LinearScale::new().domain(-24.0, 24.0).range(0.0, 200.0);

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .child("Axes Demo"),
            )
            // Bottom axis
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Bottom Axis (Linear 0-100)"),
                    )
                    .child(render_axis(
                        &x_scale,
                        &AxisConfig::bottom().with_ticks(10),
                        400.0,
                        &theme,
                    )),
            )
            // Top axis with formatter
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Top Axis (Log 20Hz-20kHz)"),
                    )
                    .child(render_axis(
                        &freq_scale,
                        &AxisConfig::top().with_ticks(10).with_formatter(|f| {
                            if f >= 1000.0 {
                                format!("{:.0}k", f / 1000.0)
                            } else {
                                format!("{:.0}", f)
                            }
                        }),
                        400.0,
                        &theme,
                    )),
            )
            // Left/Right axes
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Left & Right Axes (dB scale)"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .child(render_axis(
                                &db_scale,
                                &AxisConfig::left().with_ticks(9).with_formatter(|db| {
                                    if db > 0.0 {
                                        format!("+{:.0}", db)
                                    } else {
                                        format!("{:.0}", db)
                                    }
                                }),
                                200.0,
                                &theme,
                            ))
                            .child(
                                div()
                                    .w(px(150.0))
                                    .h(px(200.0))
                                    .bg(rgb(0xf0f0f0))
                                    .rounded_md(),
                            )
                            .child(render_axis(
                                &db_scale,
                                &AxisConfig::right().with_ticks(9),
                                200.0,
                                &theme,
                            )),
                    ),
            )
    }

    fn render_bar_chart_demo(&self) -> Div {
        let theme = DefaultAxisTheme;
        let x_scale = LinearScale::new().domain(0.0, 6.0).range(0.0, 500.0);
        let y_scale = LinearScale::new().domain(0.0, 100.0).range(0.0, 250.0);
        let scheme = ColorScheme::category10();

        let data = vec![
            BarDatum::new("Mon", 45.0),
            BarDatum::new("Tue", 68.0),
            BarDatum::new("Wed", 55.0),
            BarDatum::new("Thu", 82.0),
            BarDatum::new("Fri", 70.0),
            BarDatum::new("Sat", 38.0),
        ];

        let mixed_data = vec![
            BarDatum::new("A", 30.0),
            BarDatum::new("B", -15.0),
            BarDatum::new("C", 45.0),
            BarDatum::new("D", -25.0),
            BarDatum::new("E", 60.0),
        ];
        let mixed_y_scale = LinearScale::new().domain(-30.0, 70.0).range(0.0, 250.0);
        let mixed_x_scale = LinearScale::new().domain(0.0, 5.0).range(0.0, 500.0);

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .child("Bar Charts Demo"),
            )
            // Simple bar chart
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Simple Bar Chart"),
                    )
                    .child(
                        div()
                            .flex()
                            .child(render_axis(
                                &y_scale,
                                &AxisConfig::left().with_ticks(5),
                                250.0,
                                &theme,
                            ))
                            .child(
                                div()
                                    .w(px(500.0))
                                    .h(px(250.0))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .border_1()
                                    .border_color(rgb(0xcccccc))
                                    .child(render_grid(
                                        &x_scale,
                                        &y_scale,
                                        &GridConfig::lines_only().with_line_opacity(0.2),
                                        500.0,
                                        250.0,
                                        &theme,
                                    ))
                                    .child(render_bars(
                                        &x_scale,
                                        &y_scale,
                                        &data,
                                        500.0,
                                        250.0,
                                        &BarConfig::new()
                                            .fill_color(scheme.color(0))
                                            .opacity(0.85),
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .ml(px(60.0))
                            .child(render_axis(
                                &x_scale,
                                &AxisConfig::bottom().with_ticks(6),
                                500.0,
                                &theme,
                            )),
                    ),
            )
            // Mixed positive/negative
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Mixed Positive/Negative Values"),
                    )
                    .child(
                        div()
                            .flex()
                            .child(render_axis(
                                &mixed_y_scale,
                                &AxisConfig::left().with_ticks(7).with_formatter(|v| {
                                    if v > 0.0 {
                                        format!("+{:.0}", v)
                                    } else {
                                        format!("{:.0}", v)
                                    }
                                }),
                                250.0,
                                &theme,
                            ))
                            .child(
                                div()
                                    .w(px(500.0))
                                    .h(px(250.0))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .border_1()
                                    .border_color(rgb(0xcccccc))
                                    .child(render_grid(
                                        &mixed_x_scale,
                                        &mixed_y_scale,
                                        &GridConfig::with_lines(),
                                        500.0,
                                        250.0,
                                        &theme,
                                    ))
                                    .child(render_bars(
                                        &mixed_x_scale,
                                        &mixed_y_scale,
                                        &mixed_data,
                                        500.0,
                                        250.0,
                                        &BarConfig::new()
                                            .fill_color(scheme.color(2))
                                            .bar_gap(4.0),
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .ml(px(60.0))
                            .child(render_axis(
                                &mixed_x_scale,
                                &AxisConfig::bottom().with_ticks(5),
                                500.0,
                                &theme,
                            )),
                    ),
            )
    }

    fn render_line_chart_demo(&self) -> Div {
        let theme = DefaultAxisTheme;
        let x_scale = LinearScale::new().domain(0.0, 100.0).range(0.0, 500.0);
        let y_scale = LinearScale::new().domain(0.0, 100.0).range(0.0, 250.0);
        let scheme = ColorScheme::category10();

        let data = vec![
            LinePoint::new(0.0, 20.0),
            LinePoint::new(20.0, 45.0),
            LinePoint::new(40.0, 35.0),
            LinePoint::new(60.0, 75.0),
            LinePoint::new(80.0, 60.0),
            LinePoint::new(100.0, 85.0),
        ];

        let series1 = vec![
            LinePoint::new(0.0, 25.0),
            LinePoint::new(25.0, 50.0),
            LinePoint::new(50.0, 40.0),
            LinePoint::new(75.0, 70.0),
            LinePoint::new(100.0, 65.0),
        ];

        let series2 = vec![
            LinePoint::new(0.0, 55.0),
            LinePoint::new(25.0, 30.0),
            LinePoint::new(50.0, 60.0),
            LinePoint::new(75.0, 45.0),
            LinePoint::new(100.0, 75.0),
        ];

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .child("Line Charts Demo"),
            )
            // Linear with points
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Linear with Points"),
                    )
                    .child(
                        div()
                            .flex()
                            .child(render_axis(
                                &y_scale,
                                &AxisConfig::left().with_ticks(5),
                                250.0,
                                &theme,
                            ))
                            .child(
                                div()
                                    .w(px(500.0))
                                    .h(px(250.0))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .border_1()
                                    .border_color(rgb(0xcccccc))
                                    .child(render_grid(
                                        &x_scale,
                                        &y_scale,
                                        &GridConfig::dots_only(),
                                        500.0,
                                        250.0,
                                        &theme,
                                    ))
                                    .child(render_line(
                                        &x_scale,
                                        &y_scale,
                                        &data,
                                        &LineConfig::new()
                                            .stroke_color(scheme.color(1))
                                            .curve(CurveType::Linear)
                                            .show_points(true)
                                            .point_radius(4.0)
                                            .point_fill_color(D3Color::from_hex(0xffffff)),
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .ml(px(60.0))
                            .child(render_axis(
                                &x_scale,
                                &AxisConfig::bottom().with_ticks(5),
                                500.0,
                                &theme,
                            )),
                    ),
            )
            // Multiple series
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Multiple Series"),
                    )
                    .child(
                        div()
                            .flex()
                            .child(render_axis(
                                &y_scale,
                                &AxisConfig::left().with_ticks(5),
                                250.0,
                                &theme,
                            ))
                            .child(
                                div()
                                    .w(px(500.0))
                                    .h(px(250.0))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .border_1()
                                    .border_color(rgb(0xcccccc))
                                    .child(render_grid(
                                        &x_scale,
                                        &y_scale,
                                        &GridConfig::lines_only().with_line_opacity(0.2),
                                        500.0,
                                        250.0,
                                        &theme,
                                    ))
                                    .child(render_line(
                                        &x_scale,
                                        &y_scale,
                                        &series1,
                                        &LineConfig::new()
                                            .stroke_color(scheme.color(4))
                                            .curve(CurveType::Linear)
                                            .show_points(true)
                                            .point_radius(4.0),
                                    ))
                                    .child(render_line(
                                        &x_scale,
                                        &y_scale,
                                        &series2,
                                        &LineConfig::new()
                                            .stroke_color(scheme.color(6))
                                            .curve(CurveType::Linear)
                                            .show_points(true)
                                            .point_radius(4.0),
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .ml(px(60.0))
                            .child(render_axis(
                                &x_scale,
                                &AxisConfig::bottom().with_ticks(5),
                                500.0,
                                &theme,
                            )),
                    ),
            )
    }

    fn render_scatter_demo(&self) -> Div {
        let theme = DefaultAxisTheme;
        let x_scale = LinearScale::new().domain(0.0, 100.0).range(0.0, 500.0);
        let y_scale = LinearScale::new().domain(0.0, 100.0).range(0.0, 250.0);
        let scheme = ColorScheme::category10();

        let data1 = vec![
            ScatterPoint::new(10.0, 20.0),
            ScatterPoint::new(25.0, 45.0),
            ScatterPoint::new(35.0, 30.0),
            ScatterPoint::new(50.0, 75.0),
            ScatterPoint::new(65.0, 55.0),
            ScatterPoint::new(75.0, 85.0),
            ScatterPoint::new(85.0, 65.0),
            ScatterPoint::new(90.0, 90.0),
        ];

        let cluster1: Vec<_> = (0..15)
            .map(|i| {
                let angle = i as f64 * 0.4;
                ScatterPoint::new(30.0 + angle.cos() * 15.0, 30.0 + angle.sin() * 15.0)
            })
            .collect();

        let cluster2: Vec<_> = (0..15)
            .map(|i| {
                let angle = i as f64 * 0.5;
                ScatterPoint::new(70.0 + angle.cos() * 12.0, 70.0 + angle.sin() * 12.0)
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .child("Scatter Plots Demo"),
            )
            // Simple scatter
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Simple Scatter Plot"),
                    )
                    .child(
                        div()
                            .flex()
                            .child(render_axis(
                                &y_scale,
                                &AxisConfig::left().with_ticks(5),
                                250.0,
                                &theme,
                            ))
                            .child(
                                div()
                                    .w(px(500.0))
                                    .h(px(250.0))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .border_1()
                                    .border_color(rgb(0xcccccc))
                                    .child(render_grid(
                                        &x_scale,
                                        &y_scale,
                                        &GridConfig::dots_only(),
                                        500.0,
                                        250.0,
                                        &theme,
                                    ))
                                    .child(render_scatter(
                                        &x_scale,
                                        &y_scale,
                                        &data1,
                                        &ScatterConfig::new()
                                            .fill_color(scheme.color(0))
                                            .point_radius(6.0)
                                            .opacity(0.8),
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .ml(px(60.0))
                            .child(render_axis(
                                &x_scale,
                                &AxisConfig::bottom().with_ticks(5),
                                500.0,
                                &theme,
                            )),
                    ),
            )
            // Clusters
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Multiple Series (2 clusters)"),
                    )
                    .child(
                        div()
                            .flex()
                            .child(render_axis(
                                &y_scale,
                                &AxisConfig::left().with_ticks(5),
                                250.0,
                                &theme,
                            ))
                            .child(
                                div()
                                    .w(px(500.0))
                                    .h(px(250.0))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .border_1()
                                    .border_color(rgb(0xcccccc))
                                    .child(render_grid(
                                        &x_scale,
                                        &y_scale,
                                        &GridConfig::with_lines(),
                                        500.0,
                                        250.0,
                                        &theme,
                                    ))
                                    .child(render_scatter(
                                        &x_scale,
                                        &y_scale,
                                        &cluster1,
                                        &ScatterConfig::new()
                                            .fill_color(scheme.color(4))
                                            .point_radius(5.0)
                                            .stroke_color(D3Color::from_hex(0xffffff))
                                            .stroke_width(1.5),
                                    ))
                                    .child(render_scatter(
                                        &x_scale,
                                        &y_scale,
                                        &cluster2,
                                        &ScatterConfig::new()
                                            .fill_color(scheme.color(6))
                                            .point_radius(5.0)
                                            .stroke_color(D3Color::from_hex(0xffffff))
                                            .stroke_width(1.5),
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .ml(px(60.0))
                            .child(render_axis(
                                &x_scale,
                                &AxisConfig::bottom().with_ticks(5),
                                500.0,
                                &theme,
                            )),
                    ),
            )
    }

    fn render_contours_demo(&mut self, cx: &mut Context<Self>) -> Div {
        // Get entity handle for use in slider callbacks
        let entity = cx.entity().clone();

        // Use state values
        let grid_size = self.contour_grid_size;
        let num_levels = self.contour_num_levels;
        let peak1_x = self.contour_peak1_x as f64;
        let peak1_y = self.contour_peak1_y as f64;
        let peak2_x = self.contour_peak2_x as f64;
        let peak2_y = self.contour_peak2_y as f64;
        let bandwidth = self.density_bandwidth as f64;
        let num_points = self.density_num_points;

        // Generate a 2D Gaussian surface for contour demonstration
        let mut values = vec![0.0; grid_size * grid_size];

        // Create a surface with two peaks
        for j in 0..grid_size {
            for i in 0..grid_size {
                let x = (i as f64 / grid_size as f64) * 2.0 - 1.0;
                let y = (j as f64 / grid_size as f64) * 2.0 - 1.0;

                // Two Gaussian peaks using state parameters
                let peak1 = (-((x - peak1_x).powi(2) + (y - peak1_y).powi(2)) / 0.1).exp();
                let peak2 = 0.7 * (-((x - peak2_x).powi(2) + (y - peak2_y).powi(2)) / 0.15).exp();

                values[j * grid_size + i] = peak1 + peak2;
            }
        }

        // Generate contours at various thresholds
        let generator = ContourGenerator::new(grid_size, grid_size);

        let thresholds: Vec<f64> = (1..=num_levels)
            .map(|i| i as f64 / (num_levels + 1) as f64)
            .collect();
        let contours = generator.contours(&values, &thresholds);

        // Scales for the Gaussian surface plot
        let x_scale_gaussian = LinearScale::new()
            .domain(0.0, grid_size as f64)
            .range(0.0, 400.0);
        let y_scale_gaussian = LinearScale::new()
            .domain(0.0, grid_size as f64)
            .range(0.0, 300.0);

        // Config with viridis color scale
        let gaussian_config = ContourConfig::new()
            .stroke_width(1.5)
            .fill(true)
            .fill_opacity(0.4)
            .color_scale(viridis_color_scale());

        // Generate density estimation from points
        let points: Vec<(f64, f64)> = (0..num_points)
            .map(|i| {
                let angle = i as f64 * 0.1;
                let r = 0.3 + 0.2 * (i as f64 * 0.05).sin();
                (0.5 + r * angle.cos(), 0.5 + r * angle.sin())
            })
            .collect();

        let density_grid_size = 30;
        let density_estimator = DensityEstimator::new()
            .size(density_grid_size, density_grid_size)
            .x(0.0, 1.0)
            .y(0.0, 1.0)
            .bandwidth(bandwidth);

        let density_grid = density_estimator.estimate(&points);
        let density_max = density_grid.iter().cloned().fold(0.0_f64, f64::max);

        let density_generator = ContourGenerator::new(density_grid_size, density_grid_size);

        let density_thresholds: Vec<f64> = (1..=5)
            .map(|i| density_max * (i as f64 / 6.0))
            .collect();
        let density_contours = density_generator.contours(&density_grid, &density_thresholds);

        // Scales for the density plot
        let x_scale_density = LinearScale::new()
            .domain(0.0, density_grid_size as f64)
            .range(0.0, 300.0);
        let y_scale_density = LinearScale::new()
            .domain(0.0, density_grid_size as f64)
            .range(0.0, 300.0);

        // Config with heat color scale
        let density_config = ContourConfig::new()
            .stroke_width(1.5)
            .fill(true)
            .fill_opacity(0.5)
            .color_scale(heat_color_scale());

        div()
            .flex()
            .gap_8()
            // Left side: Visualizations
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .child("Contours Demo"),
                    )
                    // Marching Squares Contours
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Marching Squares (Gaussian Surface)"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x666666))
                                    .child("Contour lines from a 2D scalar field"),
                            )
                            .child(
                                div()
                                    .w(px(400.0))
                                    .h(px(300.0))
                                    .bg(rgb(0xf5f5f5))
                                    .border_1()
                                    .border_color(rgb(0xcccccc))
                                    .relative()
                                    .child(render_contour(
                                        contours.clone(),
                                        &x_scale_gaussian,
                                        &y_scale_gaussian,
                                        &gaussian_config,
                                    ).height(px(300.0))),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .mt_2()
                                    .text_xs()
                                    .text_color(rgb(0x666666))
                                    .child("Viridis color scale: low → high"),
                            ),
                    )
                    // Density Estimation
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Kernel Density Estimation"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x666666))
                                    .child("Density contours from point data"),
                            )
                            .child(
                                div()
                                    .w(px(300.0))
                                    .h(px(300.0))
                                    .bg(rgb(0x1a1a1a))
                                    .border_1()
                                    .border_color(rgb(0x333333))
                                    .relative()
                                    .child(render_contour(
                                        density_contours.into_iter().collect::<Vec<_>>(),
                                        &x_scale_density,
                                        &y_scale_density,
                                        &density_config,
                                    ).height(px(300.0)))
                                    // Overlay the original points
                                    .children(points.iter().map(|(x, y)| {
                                        div()
                                            .absolute()
                                            .left(px((*x * 300.0 - 2.0) as f32))
                                            .top(px(((1.0 - *y) * 300.0 - 2.0) as f32))
                                            .w(px(4.0))
                                            .h(px(4.0))
                                            .rounded_full()
                                            .bg(rgba(0xffffffaa))
                                    })),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .mt_2()
                                    .text_xs()
                                    .text_color(rgb(0x666666))
                                    .child("Heat color scale with point overlay"),
                            ),
                    ),
            )
            // Right side: Controls
            .child(
                div()
                    .w(px(280.0))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_4()
                    .bg(rgb(0xf8f8f8))
                    .border_1()
                    .border_color(rgb(0xe0e0e0))
                    .rounded_lg()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x333333))
                            .child("Controls"),
                    )
                    // Gaussian Surface Controls
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0x555555))
                                    .child("Gaussian Surface"),
                            )
                            .child({
                                let entity = entity.clone();
                                Slider::new("grid-size")
                                    .label("Grid Size")
                                    .value(self.contour_grid_size as f32)
                                    .min(20.0)
                                    .max(100.0)
                                    .step(10.0)
                                    .show_value(true)
                                    .width(220.0)
                                    .on_change(move |value, _window, cx| {
                                        entity.update(cx, |this, _| {
                                            this.contour_grid_size = value as usize;
                                        });
                                    })
                            })
                            .child({
                                let entity = entity.clone();
                                Slider::new("num-levels")
                                    .label("Contour Levels")
                                    .value(self.contour_num_levels as f32)
                                    .min(2.0)
                                    .max(10.0)
                                    .step(1.0)
                                    .show_value(true)
                                    .width(220.0)
                                    .on_change(move |value, _window, cx| {
                                        entity.update(cx, |this, _| {
                                            this.contour_num_levels = value as usize;
                                        });
                                    })
                            })
                            .child({
                                let entity = entity.clone();
                                Slider::new("peak1-x")
                                    .label("Peak 1 X")
                                    .value(self.contour_peak1_x)
                                    .min(-1.0)
                                    .max(1.0)
                                    .step(0.1)
                                    .show_value(true)
                                    .width(220.0)
                                    .on_change(move |value, _window, cx| {
                                        entity.update(cx, |this, _| {
                                            this.contour_peak1_x = value;
                                        });
                                    })
                            })
                            .child({
                                let entity = entity.clone();
                                Slider::new("peak1-y")
                                    .label("Peak 1 Y")
                                    .value(self.contour_peak1_y)
                                    .min(-1.0)
                                    .max(1.0)
                                    .step(0.1)
                                    .show_value(true)
                                    .width(220.0)
                                    .on_change(move |value, _window, cx| {
                                        entity.update(cx, |this, _| {
                                            this.contour_peak1_y = value;
                                        });
                                    })
                            })
                            .child({
                                let entity = entity.clone();
                                Slider::new("peak2-x")
                                    .label("Peak 2 X")
                                    .value(self.contour_peak2_x)
                                    .min(-1.0)
                                    .max(1.0)
                                    .step(0.1)
                                    .show_value(true)
                                    .width(220.0)
                                    .on_change(move |value, _window, cx| {
                                        entity.update(cx, |this, _| {
                                            this.contour_peak2_x = value;
                                        });
                                    })
                            })
                            .child({
                                let entity = entity.clone();
                                Slider::new("peak2-y")
                                    .label("Peak 2 Y")
                                    .value(self.contour_peak2_y)
                                    .min(-1.0)
                                    .max(1.0)
                                    .step(0.1)
                                    .show_value(true)
                                    .width(220.0)
                                    .on_change(move |value, _window, cx| {
                                        entity.update(cx, |this, _| {
                                            this.contour_peak2_y = value;
                                        });
                                    })
                            }),
                    )
                    // Density Estimation Controls
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .mt_4()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0x555555))
                                    .child("Density Estimation"),
                            )
                            .child({
                                let entity = entity.clone();
                                Slider::new("bandwidth")
                                    .label("Bandwidth")
                                    .value(self.density_bandwidth)
                                    .min(0.02)
                                    .max(0.2)
                                    .step(0.02)
                                    .show_value(true)
                                    .width(220.0)
                                    .on_change(move |value, _window, cx| {
                                        entity.update(cx, |this, _| {
                                            this.density_bandwidth = value;
                                        });
                                    })
                            })
                            .child({
                                let entity = entity.clone();
                                Slider::new("num-points")
                                    .label("Number of Points")
                                    .value(self.density_num_points as f32)
                                    .min(20.0)
                                    .max(200.0)
                                    .step(10.0)
                                    .show_value(true)
                                    .width(220.0)
                                    .on_change(move |value, _window, cx| {
                                        entity.update(cx, |this, _| {
                                            this.density_num_points = value as usize;
                                        });
                                    })
                            }),
                    )
                    // Statistics
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .mt_4()
                            .p_3()
                            .bg(rgb(0xffffff))
                            .border_1()
                            .border_color(rgb(0xe0e0e0))
                            .rounded_md()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0x888888))
                                    .child("STATISTICS"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x333333))
                                    .child(format!("Grid: {}x{}", grid_size, grid_size)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x333333))
                                    .child(format!("Levels: {}", num_levels)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x333333))
                                    .child(format!(
                                        "Rings: {}",
                                        contours.iter().map(|c| c.coordinates.len()).sum::<usize>()
                                    )),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x333333))
                                    .child(format!("Points: {}", num_points)),
                            ),
                    ),
            )
    }


    fn render_colors_demo(&self) -> Div {
        let category10 = ColorScheme::category10();

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .child("Colors Demo"),
            )
            // Category10
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Category10 Color Scheme"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .children((0..10).map(|i| {
                                let color = category10.color(i);
                                div()
                                    .w(px(40.0))
                                    .h(px(40.0))
                                    .rounded_md()
                                    .bg(color.to_rgba())
                            })),
                    ),
            )
            // Interpolation
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Color Interpolation (Red -> Blue)"),
                    )
                    .child(
                        div().flex().gap_1().children((0..20).map(|i| {
                            let t = i as f32 / 19.0;
                            let red = D3Color::rgb(255, 0, 0);
                            let blue = D3Color::rgb(0, 0, 255);
                            let color = red.interpolate(&blue, t);
                            div().w(px(20.0)).h(px(40.0)).bg(color.to_rgba())
                        })),
                    ),
            )
            // HSL Interpolation
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("HSL Gradient (Hue 0-360)"),
                    )
                    .child(
                        div().flex().gap_1().children((0..36).map(|i| {
                            let hue = i as f32 * 10.0;
                            let color = D3Color::from_hsl(hue, 0.8, 0.5);
                            div().w(px(12.0)).h(px(40.0)).bg(color.to_rgba())
                        })),
                    ),
            )
            // Lighten/Darken
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Lighten / Darken"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .children((0..11).map(|i| {
                                let base = D3Color::rgb(0, 122, 204);
                                let amount = (i as f32 - 5.0) / 5.0;
                                let color = if amount < 0.0 {
                                    base.darken(-amount)
                                } else {
                                    base.lighten(amount)
                                };
                                div().w(px(36.0)).h(px(40.0)).bg(color.to_rgba())
                            })),
                    ),
            )
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

// Menu actions
actions!(showcase, [Quit]);

fn main() {
    Application::new().run(|cx: &mut App| {
        // Register quit action
        cx.on_action::<Quit>(|_action, cx| {
            cx.quit();
        });

        // Set up menu bar
        cx.set_menus(vec![Menu {
            name: "d3rs Showcase".into(),
            items: vec![MenuItem::action("Quit d3rs Showcase", Quit)],
        }]);

        // Bind Cmd+Q to quit
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);

        let bounds = Bounds::centered(None, size(px(1000.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("d3rs Showcase".into()),
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
