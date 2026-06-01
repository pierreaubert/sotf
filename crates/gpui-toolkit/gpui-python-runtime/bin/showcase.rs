//! GPUI Python Runtime Showcase.

use gpui::*;
use gpui_design::{DesignExt, DesignSystem};
use gpui_miniapp::{MiniApp, MiniAppConfig};
use gpui_px::{ColorScale, ScaleType, bar, heatmap, line, scatter};
use gpui_python_runtime::gpui_adapter::Gpui3DCache;
use gpui_python_runtime::{
    AxisLabels, CameraSpec, ColorRgba, LinesSpec, OrbitCameraSpec, Point3, ScalarRange, SurfaceSpec,
};
use gpui_ui_kit::theme::{Theme, ThemeExt};
use std::f64::consts::TAU;

fn main() {
    MiniApp::run(
        MiniAppConfig::new("GPUI Python Runtime Showcase")
            .size(1240.0, 820.0)
            .with_theme(true)
            .scrollable(false),
        |cx| cx.new(PythonShowcase::new),
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ShowcaseSection {
    #[default]
    Surface3D,
    Lines3D,
    PxCharts,
    MixedScene,
}

impl ShowcaseSection {
    fn all() -> &'static [Self] {
        &[
            Self::Surface3D,
            Self::Lines3D,
            Self::PxCharts,
            Self::MixedScene,
        ]
    }

    fn label(self) -> &'static str {
        match self {
            Self::Surface3D => "3D Surface",
            Self::Lines3D => "3D Lines",
            Self::PxCharts => "gpui-px Charts",
            Self::MixedScene => "Scene Specs",
        }
    }
}

struct PythonShowcase {
    current_section: ShowcaseSection,
    gpui_3d: Gpui3DCache,
    scatter_x: Vec<f64>,
    scatter_y: Vec<f64>,
    line_x: Vec<f64>,
    line_y: Vec<f64>,
    heatmap_z: Vec<f64>,
    heatmap_size: usize,
    bar_categories: Vec<&'static str>,
    bar_values: Vec<f64>,
}

impl PythonShowcase {
    fn new(_cx: &mut Context<Self>) -> Self {
        let (scatter_x, scatter_y) = generate_scatter_data();
        let (line_x, line_y) = generate_frequency_response();
        let heatmap_size = 24;
        let heatmap_z = generate_heatmap_data(heatmap_size);

        Self {
            current_section: ShowcaseSection::default(),
            gpui_3d: Gpui3DCache::new(),
            scatter_x,
            scatter_y,
            line_x,
            line_y,
            heatmap_z,
            heatmap_size,
            bar_categories: vec!["Surface", "Lines", "Mesh", "Light", "Callback"],
            bar_values: vec![42.0, 31.0, 18.0, 8.0, 5.0],
        }
    }

    fn render_sidebar(&mut self, theme: &Theme, ds: &DesignSystem, cx: &mut Context<Self>) -> Div {
        let current = self.current_section;

        div()
            .w(px(230.0))
            .h_full()
            .flex()
            .flex_col()
            .gap(px(ds.spacing.control_gap))
            .p(px(ds.spacing.card_padding))
            .bg(theme.surface)
            .border_r_1()
            .border_color(theme.border)
            .child(
                div()
                    .mb(px(ds.spacing.section_gap))
                    .flex()
                    .flex_col()
                    .gap(px(ds.spacing.grid_unit))
                    .child(
                        div()
                            .text_size(px(ds.typography.large_size))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.text_primary)
                            .child("Scene3D"),
                    )
                    .child(
                        div()
                            .text_size(px(ds.typography.small_size))
                            .text_color(theme.text_muted)
                            .child("Python declarations, Rust renderers"),
                    ),
            )
            .children(ShowcaseSection::all().iter().copied().map(|section| {
                let selected = section == current;
                let bg = if selected {
                    theme.accent
                } else {
                    theme.surface
                };
                let hover_bg = if selected {
                    theme.accent_hover
                } else {
                    theme.surface_hover
                };
                let text = if selected {
                    theme.text_on_accent
                } else {
                    theme.text_primary
                };

                div()
                    .id(ElementId::Name(section.label().into()))
                    .px(px(ds.spacing.control_padding_x))
                    .py(px(ds.spacing.control_padding_y))
                    .rounded(px(ds.corners.md))
                    .cursor_pointer()
                    .bg(bg)
                    .hover(move |style| style.bg(hover_bg))
                    .text_color(text)
                    .child(section.label())
                    .on_click(cx.listener(move |this, _, _window, _cx| {
                        this.current_section = section;
                    }))
            }))
    }

    fn render_content(&mut self, theme: &Theme, ds: &DesignSystem) -> impl IntoElement {
        let content = match self.current_section {
            ShowcaseSection::Surface3D => self.render_surface_3d(theme, ds),
            ShowcaseSection::Lines3D => self.render_lines_3d(theme, ds),
            ShowcaseSection::PxCharts => self.render_px_charts(theme, ds),
            ShowcaseSection::MixedScene => self.render_scene_specs(theme, ds),
        };

        div()
            .id("python-showcase-content")
            .flex_1()
            .h_full()
            .overflow_y_scroll()
            .bg(theme.background)
            .p(px(ds.spacing.section_gap * 1.5))
            .child(content)
    }

    fn render_surface_3d(&mut self, theme: &Theme, ds: &DesignSystem) -> Div {
        let spec = build_surface_spec();
        let element = self
            .gpui_3d
            .surface_element(&spec)
            .expect("surface spec is static and validated");

        div()
            .flex()
            .flex_col()
            .gap(px(ds.spacing.section_gap))
            .child(section_header(
                "3D Surface",
                "Rust-owned wgpu surface from a Python-style spec",
                theme,
                ds,
            ))
            .child(
                div()
                    .w(px(760.0))
                    .h(px(480.0))
                    .bg(theme.surface)
                    .rounded(px(ds.corners.md))
                    .border_1()
                    .border_color(theme.border)
                    .child(element),
            )
            .child(spec_summary(
                theme,
                ds,
                &[
                    ("id", spec.id.as_str()),
                    ("grid", "10 x 7"),
                    ("camera", "orbit distance 3.8"),
                    ("resource path", "Surface3DElement"),
                ],
            ))
    }

    fn render_lines_3d(&mut self, theme: &Theme, ds: &DesignSystem) -> Div {
        let spec = build_lines_spec();
        let element = self
            .gpui_3d
            .lines_element(&spec)
            .expect("line spec is static and validated");

        div()
            .flex()
            .flex_col()
            .gap(px(ds.spacing.section_gap))
            .child(section_header(
                "3D Lines",
                "Line strips using the same orbit camera model",
                theme,
                ds,
            ))
            .child(
                div()
                    .w(px(700.0))
                    .h(px(440.0))
                    .rounded(px(ds.corners.md))
                    .border_1()
                    .border_color(theme.border)
                    .overflow_hidden()
                    .child(element),
            )
            .child(spec_summary(
                theme,
                ds,
                &[
                    ("id", spec.id.as_str()),
                    ("strips", "helix + xyz axes"),
                    ("resource path", "Lines3DElement"),
                ],
            ))
    }

    fn render_px_charts(&self, theme: &Theme, ds: &DesignSystem) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(ds.spacing.section_gap))
            .child(section_header(
                "gpui-px Charts",
                "A compact chart set embedded beside scene3d",
                theme,
                ds,
            ))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(ds.spacing.section_gap))
                    .child(
                        scatter(&self.scatter_x, &self.scatter_y)
                            .title("Callback Latency")
                            .color(0x1f77b4)
                            .point_radius(4.0)
                            .size(360.0, 260.0)
                            .build()
                            .expect("static scatter data"),
                    )
                    .child(
                        line(&self.line_x, &self.line_y)
                            .title("Frequency Response")
                            .color(0xff7f0e)
                            .x_scale(ScaleType::Log)
                            .stroke_width(2.0)
                            .size(360.0, 260.0)
                            .build()
                            .expect("static line data"),
                    )
                    .child(
                        bar(&self.bar_categories, &self.bar_values)
                            .title("Scene Nodes")
                            .color(0x2ca02c)
                            .size(360.0, 260.0)
                            .build()
                            .expect("static bar data"),
                    )
                    .child(
                        heatmap(&self.heatmap_z, self.heatmap_size, self.heatmap_size)
                            .title("Upload Activity")
                            .color_scale(ColorScale::Viridis)
                            .size(360.0, 260.0)
                            .build()
                            .expect("static heatmap data"),
                    ),
            )
    }

    fn render_scene_specs(&mut self, theme: &Theme, ds: &DesignSystem) -> Div {
        let surface = build_surface_spec();
        let lines = build_lines_spec();

        div()
            .flex()
            .flex_col()
            .gap(px(ds.spacing.section_gap))
            .child(section_header(
                "Scene Specs",
                "Stable ids drive retained GPU resources",
                theme,
                ds,
            ))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(ds.spacing.section_gap))
                    .child(metric_tile(
                        "Surface samples",
                        surface.z.values.len(),
                        theme,
                        ds,
                    ))
                    .child(metric_tile("Line points", 80 + 2 + 2 + 2, theme, ds))
                    .child(metric_tile("Cache entries", 2, theme, ds))
                    .child(metric_tile("Python calls while idle", 0, theme, ds)),
            )
            .child(spec_summary(
                theme,
                ds,
                &[
                    ("surface id", surface.id.as_str()),
                    ("lines id", lines.id.as_str()),
                    ("dirty split", "geometry / material / camera"),
                    ("raw wgpu", "private"),
                ],
            ))
            .child(
                div()
                    .flex()
                    .gap(px(ds.spacing.section_gap))
                    .child({
                        let surface_element = self
                            .gpui_3d
                            .surface_element(&surface)
                            .expect("surface spec");
                        div()
                            .w(px(420.0))
                            .h(px(280.0))
                            .rounded(px(ds.corners.md))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .child(surface_element)
                    })
                    .child({
                        let lines_element = self.gpui_3d.lines_element(&lines).expect("lines spec");
                        div()
                            .w(px(420.0))
                            .h(px(280.0))
                            .rounded(px(ds.corners.md))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .child(lines_element)
                    }),
            )
    }
}

impl Render for PythonShowcase {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let ds = cx.design();

        div()
            .size_full()
            .flex()
            .flex_row()
            .child(self.render_sidebar(&theme, &ds, cx))
            .child(self.render_content(&theme, &ds))
    }
}

fn section_header(title: &str, subtitle: &str, theme: &Theme, ds: &DesignSystem) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(ds.spacing.grid_unit))
        .child(
            div()
                .text_size(px(ds.typography.large_size))
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_primary)
                .child(title.to_string()),
        )
        .child(
            div()
                .text_size(px(ds.typography.small_size))
                .text_color(theme.text_secondary)
                .child(subtitle.to_string()),
        )
}

fn spec_summary(theme: &Theme, ds: &DesignSystem, rows: &[(&str, &str)]) -> Div {
    div()
        .w(px(760.0))
        .flex()
        .flex_col()
        .gap(px(ds.spacing.grid_unit))
        .p(px(ds.spacing.card_padding))
        .bg(theme.surface)
        .rounded(px(ds.corners.md))
        .border_1()
        .border_color(theme.border)
        .children(rows.iter().map(|(label, value)| {
            div()
                .flex()
                .justify_between()
                .gap(px(ds.spacing.section_gap))
                .child(
                    div()
                        .text_size(px(ds.typography.small_size))
                        .text_color(theme.text_muted)
                        .child((*label).to_string()),
                )
                .child(
                    div()
                        .text_size(px(ds.typography.small_size))
                        .text_color(theme.text_primary)
                        .child((*value).to_string()),
                )
        }))
}

fn metric_tile(label: &str, value: usize, theme: &Theme, ds: &DesignSystem) -> Div {
    div()
        .w(px(180.0))
        .p(px(ds.spacing.card_padding))
        .bg(theme.surface)
        .rounded(px(ds.corners.md))
        .border_1()
        .border_color(theme.border)
        .flex()
        .flex_col()
        .gap(px(ds.spacing.grid_unit))
        .child(
            div()
                .text_size(px(ds.typography.large_size))
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_primary)
                .child(value.to_string()),
        )
        .child(
            div()
                .text_size(px(ds.typography.small_size))
                .text_color(theme.text_muted)
                .child(label.to_string()),
        )
}

fn build_surface_spec() -> SurfaceSpec {
    let freqs = vec![
        20.0, 40.0, 80.0, 160.0, 315.0, 630.0, 1250.0, 2500.0, 5000.0, 10000.0,
    ];
    let angles = vec![-90.0, -60.0, -30.0, 0.0, 30.0, 60.0, 90.0];
    let mut z = Vec::with_capacity(freqs.len() * angles.len());

    for angle in &angles {
        let angle_weight = f64::abs(*angle) / 90.0;
        for freq in &freqs {
            let octave = f64::log2(*freq / 1000.0);
            let on_axis_ripple = 2.0 * f64::sin(octave * 2.4);
            let off_axis_rolloff = -9.0 * angle_weight * f64::max(0.0, f64::log10(*freq / 1000.0));
            z.push(on_axis_ripple + off_axis_rolloff);
        }
    }

    let mut spec = SurfaceSpec::from_flat("dispersion", z, freqs.len(), angles.len());
    spec.x = Some(freqs);
    spec.y = Some(angles);
    spec.x_log = true;
    spec.z_range = Some(ScalarRange::new(-12.0, 4.0));
    spec.labels = AxisLabels {
        x: Some("Frequency (Hz)".to_string()),
        y: Some("Angle (deg)".to_string()),
        z: Some("Level (dB)".to_string()),
        title: None,
    };
    spec.camera = Some(CameraSpec::Orbit(OrbitCameraSpec::new(3.8, 58.0, 28.0)));
    spec
}

fn build_lines_spec() -> LinesSpec {
    let mut helix = Vec::with_capacity(80);
    for index in 0..80 {
        let t = index as f64 / 79.0;
        let angle = t * 2.5 * TAU;
        let radius = 0.7 + 0.2 * f64::sin(t * TAU);
        helix.push(Point3::new(
            (radius * f64::cos(angle)) as f32,
            ((t - 0.5) * 1.8) as f32,
            (radius * f64::sin(angle)) as f32,
        ));
    }

    LinesSpec {
        id: "orbit-lines".to_string(),
        strips: vec![
            gpui_python_runtime::LineStripSpec {
                id: "helix".to_string(),
                points: helix,
                color: ColorRgba::from_hex("#7dd3fc").expect("static color"),
                width: 2.5,
            },
            gpui_python_runtime::LineStripSpec {
                id: "x-axis".to_string(),
                points: vec![Point3::new(-1.2, 0.0, 0.0), Point3::new(1.2, 0.0, 0.0)],
                color: ColorRgba::from_hex("#ef4444").expect("static color"),
                width: 1.5,
            },
            gpui_python_runtime::LineStripSpec {
                id: "y-axis".to_string(),
                points: vec![Point3::new(0.0, -1.0, 0.0), Point3::new(0.0, 1.0, 0.0)],
                color: ColorRgba::from_hex("#22c55e").expect("static color"),
                width: 1.5,
            },
            gpui_python_runtime::LineStripSpec {
                id: "z-axis".to_string(),
                points: vec![Point3::new(0.0, 0.0, -1.2), Point3::new(0.0, 0.0, 1.2)],
                color: ColorRgba::from_hex("#3b82f6").expect("static color"),
                width: 1.5,
            },
        ],
        background: Some(ColorRgba::from_hex("#0b1020").expect("static color")),
        camera: Some(CameraSpec::Orbit(OrbitCameraSpec::new(4.2, 42.0, 24.0))),
        ..LinesSpec::default()
    }
}

fn generate_scatter_data() -> (Vec<f64>, Vec<f64>) {
    let mut x = Vec::with_capacity(80);
    let mut y = Vec::with_capacity(80);
    for i in 0..80 {
        let t = i as f64 / 79.0;
        x.push(t * 100.0);
        y.push(20.0 + 28.0 * t + 8.0 * f64::sin(t * TAU * 3.0));
    }
    (x, y)
}

fn generate_frequency_response() -> (Vec<f64>, Vec<f64>) {
    let mut x = Vec::with_capacity(72);
    let mut y = Vec::with_capacity(72);
    for i in 0..72 {
        let freq = 20.0 * 10_f64.powf(i as f64 / 23.0);
        let bass_shelf = if freq < 120.0 {
            -5.0 * (120.0 - freq) / 100.0
        } else {
            0.0
        };
        let treble = if freq > 6000.0 {
            -4.0 * (freq - 6000.0) / 14000.0
        } else {
            0.0
        };
        x.push(freq);
        y.push(bass_shelf + treble + 1.2 * f64::sin(f64::log2(freq / 1000.0) * 3.0));
    }
    (x, y)
}

fn generate_heatmap_data(size: usize) -> Vec<f64> {
    let mut values = Vec::with_capacity(size * size);
    for y in 0..size {
        for x in 0..size {
            let nx = x as f64 / (size - 1) as f64 * 2.0 - 1.0;
            let ny = y as f64 / (size - 1) as f64 * 2.0 - 1.0;
            let left = f64::exp(-((nx + 0.35).powi(2) + (ny - 0.2).powi(2)) * 8.0);
            let right = 0.7 * f64::exp(-((nx - 0.4).powi(2) + (ny + 0.25).powi(2)) * 18.0);
            values.push(left + right);
        }
    }
    values
}
