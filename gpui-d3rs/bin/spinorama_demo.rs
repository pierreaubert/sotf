//! Spinorama Demo - Speaker frequency response visualization
//!
//! Demonstrates fetching and plotting speaker measurement data from spinorama.org.

use std::collections::HashMap;
use std::sync::Arc;

use autoeq::read::{
    extract_cea2034_curves_original, fetch_available_speakers, fetch_contour_data,
    fetch_directivity_data, fetch_measurement_plot_data, ContourPlotData,
};
use autoeq::{Curve, DirectivityData};
use d3rs::axis::{render_axis, AxisConfig, DefaultAxisTheme};
use d3rs::color::D3Color;
use d3rs::contour::ContourGenerator;
use d3rs::grid::{render_grid, GridConfig};
use d3rs::prelude::*;
use d3rs::shape::contour::{render_contour, viridis_color_scale, ContourConfig};
use d3rs::shape::LineConfig;
use d3rs::text::{render_vector_text, VectorFontConfig};
use gpui::prelude::*;
use gpui::{actions, deferred, *};
use gpui_ui_kit::{SelectOption, Spinner, SpinnerSize};
use tokio::runtime::Runtime;
use urlencoding;

/// CEA2034 measurement curve names in standard order
const CEA2034_CURVES: &[&str] = &[
    "On Axis",
    "Listening Window",
    "Early Reflections",
    "Sound Power",
    "Early Reflections DI",
    "Sound Power DI",
];

/// Colors for CEA2034 curves
fn cea2034_colors() -> HashMap<&'static str, D3Color> {
    let mut colors = HashMap::new();
    colors.insert("On Axis", D3Color::rgb(31, 119, 180)); // Blue
    colors.insert("Listening Window", D3Color::rgb(255, 127, 14)); // Orange
    colors.insert("Early Reflections", D3Color::rgb(44, 160, 44)); // Green
    colors.insert("Sound Power", D3Color::rgb(214, 39, 40)); // Red
    colors.insert("Early Reflections DI", D3Color::rgb(148, 103, 189)); // Purple
    colors.insert("Sound Power DI", D3Color::rgb(140, 86, 75)); // Brown
    colors
}

/// A single curve to be rendered on the frequency/SPL plot
struct PlotCurve {
    /// Data points as (frequency, value) pairs
    points: Vec<LinePoint>,
    /// Curve color
    color: D3Color,
    /// Stroke width
    stroke_width: f32,
    /// Whether this curve uses the secondary (right) Y-axis
    use_secondary_axis: bool,
}

impl PlotCurve {
    fn new(points: Vec<LinePoint>, color: D3Color) -> Self {
        Self {
            points,
            color,
            stroke_width: 2.0,
            use_secondary_axis: false,
        }
    }

    fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width;
        self
    }

    fn secondary_axis(mut self) -> Self {
        self.use_secondary_axis = true;
        self
    }
}

/// Configuration for the secondary (right) Y-axis
struct SecondaryAxisConfig {
    /// Domain for the secondary axis (min, max)
    domain: (f64, f64),
    /// Title for the secondary axis
    title: &'static str,
    /// Tick values (only values in this list will show labels)
    tick_values: Vec<f64>,
}

/// Renders a reusable frequency/SPL plot with optional secondary Y-axis
///
/// This is the common chart used for CEA2034, horizontal SPL, and vertical SPL plots.
/// All use a log frequency X-axis (20Hz-20kHz) and linear SPL Y-axis.
fn render_freq_spl_plot(
    curves: Vec<PlotCurve>,
    spl_domain: (f64, f64),
    secondary_axis: Option<SecondaryAxisConfig>,
    chart_width: f32,
    chart_height: f32,
) -> Div {
    let theme = DefaultAxisTheme;

    // Create log frequency scale (20Hz - 20kHz)
    let freq_scale = LogScale::new()
        .domain(20.0, 20000.0)
        .range(0.0, chart_width as f64);
    // Create linear SPL scale for main curves
    let spl_scale = LinearScale::new()
        .domain(spl_domain.0, spl_domain.1)
        .range(0.0, chart_height as f64);

    // Major frequency ticks (with labels and grid lines)
    let major_freq_ticks = vec![
        20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
    ];

    // Minor frequency ticks (no labels, smaller marks)
    let minor_freq_ticks: Vec<f64> = vec![
        // 20-100 range
        30.0, 40.0, 60.0, 70.0, 80.0, 90.0, // 100-1000 range
        300.0, 400.0, 600.0, 700.0, 800.0, 900.0, // 1000-10000 range
        3000.0, 4000.0, 6000.0, 7000.0, 8000.0, 9000.0,
    ];

    // Grid lines only at major frequencies
    let grid_freq_values = vec![
        50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
    ];

    // Generate SPL tick values
    let spl_step = 10.0;
    let spl_ticks: Vec<f64> = {
        let start = (spl_domain.0 / spl_step).ceil() as i32;
        let end = (spl_domain.1 / spl_step).floor() as i32;
        (start..=end).map(|i| i as f64 * spl_step).collect()
    };

    // Create secondary scale if needed
    let secondary_scale = secondary_axis.as_ref().map(|cfg| {
        LinearScale::new()
            .domain(cfg.domain.0, cfg.domain.1)
            .range(0.0, chart_height as f64)
    });

    // Separate curves by axis
    let primary_curves: Vec<&PlotCurve> = curves.iter().filter(|c| !c.use_secondary_axis).collect();
    let secondary_curves: Vec<&PlotCurve> =
        curves.iter().filter(|c| c.use_secondary_axis).collect();

    div()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_start()
                // Left Y-axis (SPL)
                .child(render_axis(
                    &spl_scale,
                    &AxisConfig::left()
                        .with_tick_values(spl_ticks)
                        .with_formatter(|v| format!("{:.0}", v))
                        .with_title("SPL (dB)"),
                    chart_height,
                    &theme,
                ))
                // Chart area
                .child(
                    div()
                        .w(px(chart_width))
                        .h(px(chart_height))
                        .relative()
                        .bg(rgb(0xf8f8f8))
                        .child(render_grid(
                            &freq_scale,
                            &spl_scale,
                            &GridConfig::with_lines()
                                .with_vertical_values(grid_freq_values.clone()),
                            chart_width,
                            chart_height,
                            &theme,
                        ))
                        // Render primary axis curves
                        .children(primary_curves.iter().filter_map(|curve| {
                            if curve.points.is_empty() {
                                return None;
                            }
                            Some(render_line(
                                &freq_scale,
                                &spl_scale,
                                &curve.points,
                                &LineConfig::new()
                                    .stroke_color(curve.color.clone())
                                    .stroke_width(curve.stroke_width)
                                    .curve(CurveType::Linear),
                            ))
                        }))
                        // Render secondary axis curves
                        .children(secondary_curves.iter().filter_map(|curve| {
                            let sec_scale = secondary_scale.as_ref()?;
                            if curve.points.is_empty() {
                                return None;
                            }
                            Some(render_line(
                                &freq_scale,
                                sec_scale,
                                &curve.points,
                                &LineConfig::new()
                                    .stroke_color(curve.color.clone())
                                    .stroke_width(curve.stroke_width)
                                    .curve(CurveType::Linear),
                            ))
                        })),
                )
                // Right Y-axis (optional, for DI curves)
                .when_some(secondary_axis, |el, cfg| {
                    let sec_scale = LinearScale::new()
                        .domain(cfg.domain.0, cfg.domain.1)
                        .range(0.0, chart_height as f64);
                    // Note: with_formatter takes a fn pointer, so we can't capture max_label_value
                    // For DI axis, we use the tick values directly and filter with max_label_value
                    // by passing only tick values up to max_label_value that should have labels
                    let axis_config = AxisConfig::right()
                        .with_tick_values(cfg.tick_values)
                        .with_formatter(|v| format!("{:.0}", v))
                        .with_title(cfg.title);
                    el.child(render_axis(&sec_scale, &axis_config, chart_height, &theme))
                }),
        )
        // Bottom axis
        .child(
            div()
                .flex()
                .child(
                    // Spacer for left axis
                    div().w(px(80.0)),
                )
                .child(render_axis(
                    &freq_scale,
                    &AxisConfig::bottom()
                        .with_tick_values(major_freq_ticks)
                        .with_minor_tick_values(minor_freq_ticks)
                        .with_minor_tick_size(3.0)
                        .with_formatter(|f| {
                            if f >= 1000.0 {
                                format!("{:.0}k", f / 1000.0)
                            } else {
                                format!("{:.0}", f)
                            }
                        })
                        .with_title("Frequency (Hz)"),
                    chart_width,
                    &theme,
                )),
        )
}

/// Contour rendering mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ContourRenderMode {
    #[default]
    Isoline,
    Surface,
}

impl ContourRenderMode {
    fn label(&self) -> &'static str {
        match self {
            Self::Isoline => "Isoline",
            Self::Surface => "Surface",
        }
    }

    fn toggle(&self) -> Self {
        match self {
            Self::Isoline => Self::Surface,
            Self::Surface => Self::Isoline,
        }
    }
}

/// View sections
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PlotSection {
    #[default]
    CEA2034,
    HorizontalSPL,
    VerticalSPL,
    Contour,
}

impl PlotSection {
    fn all() -> Vec<Self> {
        vec![
            Self::CEA2034,
            Self::HorizontalSPL,
            Self::VerticalSPL,
            Self::Contour,
        ]
    }

    fn label(&self) -> &'static str {
        match self {
            Self::CEA2034 => "CEA2034 (Spinorama)",
            Self::HorizontalSPL => "Horizontal SPL",
            Self::VerticalSPL => "Vertical SPL",
            Self::Contour => "Contour Plot",
        }
    }
}

/// Loading state for async data
#[derive(Debug, Clone, PartialEq)]
enum LoadState {
    Idle,
    Loading,
    Loaded,
    Error(String),
}

/// Main application state
struct SpinoramaApp {
    runtime: Arc<Runtime>,
    // Speaker list
    speakers: Vec<String>,
    speakers_load_state: LoadState,
    // Version list for selected speaker
    versions: Vec<String>,
    versions_load_state: LoadState,
    // Selection state
    selected_speaker: Option<String>,
    selected_version: Option<String>,
    selected_measurement: String,
    // Data state
    cea2034_curves: HashMap<String, Curve>,
    directivity_data: Option<DirectivityData>,
    contour_data: Option<ContourPlotData>,
    data_load_state: LoadState,
    // UI state
    current_section: PlotSection,
    speaker_dropdown_open: bool,
    version_dropdown_open: bool,
    section_dropdown_open: bool,
    // Contour render mode for each plot (SPL Horizontal Contour, Directivity Contour)
    contour_mode_spl: ContourRenderMode,
    contour_mode_directivity: ContourRenderMode,
}

impl SpinoramaApp {
    fn new(cx: &mut Context<Self>) -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime"),
        );

        let mut app = Self {
            runtime,
            speakers: Vec::new(),
            speakers_load_state: LoadState::Idle,
            versions: Vec::new(),
            versions_load_state: LoadState::Idle,
            selected_speaker: None,
            selected_version: None,
            selected_measurement: "CEA2034".to_string(),
            cea2034_curves: HashMap::new(),
            directivity_data: None,
            contour_data: None,
            data_load_state: LoadState::Idle,
            current_section: PlotSection::default(),
            speaker_dropdown_open: false,
            version_dropdown_open: false,
            section_dropdown_open: false,
            contour_mode_spl: ContourRenderMode::default(),
            contour_mode_directivity: ContourRenderMode::default(),
        };

        // Start loading speakers list
        app.load_speakers(cx);
        app
    }

    fn load_speakers(&mut self, cx: &mut Context<Self>) {
        self.speakers_load_state = LoadState::Loading;
        let runtime = self.runtime.clone();

        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let result: Result<Vec<String>, String> = runtime
                .spawn(async { fetch_available_speakers().await.map_err(|e| e.to_string()) })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r);

            match result {
                Ok(speakers) => {
                    println!("Loaded {} speakers", speakers.len());
                    let _ = this.update(cx, |app, cx| {
                        app.speakers = speakers;
                        app.speakers_load_state = LoadState::Loaded;
                        // Auto-select first speaker and load its versions
                        if let Some(first_speaker) = app.speakers.first().cloned() {
                            app.selected_speaker = Some(first_speaker);
                            app.load_versions(cx);
                        }
                        cx.notify();
                    });
                }
                Err(e) => {
                    println!("Error loading speakers: {}", e);
                    let _ = this.update(cx, |app, cx| {
                        app.speakers_load_state = LoadState::Error(e);
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn load_versions(&mut self, cx: &mut Context<Self>) {
        let Some(speaker) = self.selected_speaker.clone() else {
            return;
        };

        self.versions_load_state = LoadState::Loading;
        self.versions.clear();
        self.selected_version = None;
        let runtime = self.runtime.clone();

        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let result: Result<Vec<String>, String> = runtime
                .spawn({
                    let speaker = speaker.clone();
                    async move {
                        let url = format!(
                            "https://api.spinorama.org/v1/speaker/{}/versions",
                            urlencoding::encode(&speaker)
                        );
                        let response = reqwest::get(&url).await.map_err(|e| e.to_string())?;
                        if !response.status().is_success() {
                            return Err(format!("Failed to fetch versions: {}", response.status()));
                        }
                        let versions: Vec<String> =
                            response.json().await.map_err(|e| e.to_string())?;
                        Ok(versions)
                    }
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r);

            match result {
                Ok(versions) => {
                    println!("Loaded {} versions for speaker", versions.len());
                    let _ = this.update(cx, |app, cx| {
                        app.versions = versions;
                        app.versions_load_state = LoadState::Loaded;
                        // Auto-select first version and load speaker data
                        if let Some(first_version) = app.versions.first().cloned() {
                            app.selected_version = Some(first_version);
                            app.load_speaker_data(cx);
                        }
                        cx.notify();
                    });
                }
                Err(e) => {
                    println!("Error loading versions: {}", e);
                    let _ = this.update(cx, |app, cx| {
                        app.versions_load_state = LoadState::Error(e);
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn load_speaker_data(&mut self, cx: &mut Context<Self>) {
        let Some(speaker) = self.selected_speaker.clone() else {
            return;
        };
        let Some(version) = self.selected_version.clone() else {
            return;
        };

        self.data_load_state = LoadState::Loading;
        let runtime = self.runtime.clone();
        let measurement = self.selected_measurement.clone();

        cx.spawn(async move |this: WeakEntity<Self>, cx| {
            // Fetch CEA2034 data
            let cea2034_result: Result<HashMap<String, Curve>, String> = runtime
                .spawn({
                    let speaker = speaker.clone();
                    let version = version.clone();
                    let measurement = measurement.clone();
                    async move {
                        let plot_data =
                            fetch_measurement_plot_data(&speaker, &version, &measurement)
                                .await
                                .map_err(|e| e.to_string())?;
                        extract_cea2034_curves_original(&plot_data, &measurement)
                            .map_err(|e| e.to_string())
                    }
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r);

            // Fetch directivity data
            let directivity_result: Option<DirectivityData> = runtime
                .spawn({
                    let speaker = speaker.clone();
                    let version = version.clone();
                    async move { fetch_directivity_data(&speaker, &version).await.ok() }
                })
                .await
                .ok()
                .flatten();

            // Fetch contour data (SPL Horizontal Contour)
            let contour_result: Option<ContourPlotData> = runtime
                .spawn({
                    let speaker = speaker.clone();
                    let version = version.clone();
                    async move { fetch_contour_data(&speaker, &version, "horizontal").await.ok() }
                })
                .await
                .ok()
                .flatten();

            match cea2034_result {
                Ok(curves) => {
                    let _ = this.update(cx, |app, cx| {
                        app.cea2034_curves = curves;
                        app.directivity_data = directivity_result;
                        app.contour_data = contour_result;
                        app.data_load_state = LoadState::Loaded;
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.data_load_state = LoadState::Error(e);
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn render_header(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let speaker_options: Vec<SelectOption> = self
            .speakers
            .iter()
            .map(|s| SelectOption::new(s.clone(), s.clone()))
            .collect();

        let version_options: Vec<SelectOption> = self
            .versions
            .iter()
            .map(|v| SelectOption::new(v.clone(), v.clone()))
            .collect();

        let section_options: Vec<SelectOption> = PlotSection::all()
            .iter()
            .map(|s| SelectOption::new(s.label(), s.label()))
            .collect();

        let current_speaker = self.selected_speaker.clone();
        let current_version = self.selected_version.clone();
        let current_section = self.current_section.label();
        let speaker_dropdown_open = self.speaker_dropdown_open;
        let version_dropdown_open = self.version_dropdown_open;
        let section_dropdown_open = self.section_dropdown_open;
        let is_loading_speakers = self.speakers_load_state == LoadState::Loading;
        let is_loading_versions = self.versions_load_state == LoadState::Loading;
        let is_loading_data = self.data_load_state == LoadState::Loading;
        let has_speaker = self.selected_speaker.is_some();

        div()
            .w_full()
            .min_h(px(60.0))
            .bg(rgb(0x1e1e1e))
            .border_b_1()
            .border_color(rgb(0x3c3c3c))
            .flex()
            .items_center()
            .px_4()
            .py_2()
            .gap_4()
            // Speaker select
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_sm().text_color(rgb(0xcccccc)).child("Speaker:"))
                    .child(if is_loading_speakers {
                        div()
                            .id("speaker-loading")
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(Spinner::new().size(SpinnerSize::Sm))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x888888))
                                    .child("Loading..."),
                            )
                    } else {
                        self.render_speaker_dropdown(
                            speaker_options,
                            current_speaker,
                            speaker_dropdown_open,
                            cx,
                        )
                    }),
            )
            // Version select (only show if speaker is selected)
            .when(has_speaker, |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().text_sm().text_color(rgb(0xcccccc)).child("Version:"))
                        .child(if is_loading_versions {
                            div()
                                .id("version-loading")
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(Spinner::new().size(SpinnerSize::Sm))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(0x888888))
                                        .child("Loading..."),
                                )
                        } else {
                            self.render_version_dropdown(
                                version_options,
                                current_version,
                                version_dropdown_open,
                                cx,
                            )
                        }),
                )
            })
            // Plot type select
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_sm().text_color(rgb(0xcccccc)).child("Plot:"))
                    .child(self.render_section_dropdown(
                        section_options,
                        current_section,
                        section_dropdown_open,
                        cx,
                    )),
            )
            // Loading indicator
            .when(is_loading_data, |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .ml_auto()
                        .child(Spinner::new().size(SpinnerSize::Sm))
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x888888))
                                .child("Loading data..."),
                        ),
                )
            })
    }

    fn render_speaker_dropdown(
        &mut self,
        options: Vec<SelectOption>,
        current: Option<String>,
        is_open: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let entity = cx.entity().clone();
        let entity_for_toggle = cx.entity().clone();

        div()
            .relative()
            .id("speaker-dropdown-container")
            .child(
                div()
                    .id("speaker-select")
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .min_w(px(200.0))
                    .bg(rgb(0x2a2a2a))
                    .border_1()
                    .border_color(rgb(0x3a3a3a))
                    .rounded_md()
                    .cursor_pointer()
                    .text_sm()
                    .hover(|s| s.border_color(rgb(0x007acc)))
                    .child(
                        div()
                            .text_color(if current.is_some() {
                                rgb(0xffffff)
                            } else {
                                rgb(0x666666)
                            })
                            .child(
                                current
                                    .clone()
                                    .unwrap_or_else(|| "Select speaker...".into()),
                            ),
                    )
                    .child(div().text_xs().text_color(rgb(0x666666)).child("▼"))
                    .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                        println!("Speaker dropdown clicked!");
                        entity_for_toggle.update(cx, |this, cx| {
                            this.speaker_dropdown_open = !this.speaker_dropdown_open;
                            this.version_dropdown_open = false;
                            this.section_dropdown_open = false;
                            println!(
                                "Speaker dropdown open: {}, speakers count: {}",
                                this.speaker_dropdown_open,
                                this.speakers.len()
                            );
                            cx.notify();
                        });
                    }),
            )
            .when(is_open, |el| {
                el.child(
                    deferred(
                        div()
                            .id("speaker-dropdown")
                            .absolute()
                            .top_full()
                            .left_0()
                            .mt_1()
                            .w(px(300.0))
                            .max_h(px(400.0))
                            .overflow_y_scroll()
                            .bg(rgb(0x2a2a2a))
                            .border_1()
                            .border_color(rgb(0x3a3a3a))
                            .rounded_md()
                            .shadow_lg()
                            .py_1()
                            .children(options.into_iter().enumerate().map(|(i, opt)| {
                                let is_selected = current.as_ref() == Some(&opt.value.to_string());
                                let value = opt.value.to_string();
                                let entity = entity.clone();

                                div()
                                    .id(ElementId::NamedInteger("speaker-opt".into(), i as u64))
                                    .px_3()
                                    .py(px(6.0))
                                    .cursor_pointer()
                                    .text_sm()
                                    .when(is_selected, |el| {
                                        el.bg(rgb(0x007acc)).text_color(rgb(0xffffff))
                                    })
                                    .when(!is_selected, |el| {
                                        el.text_color(rgb(0xcccccc)).hover(|s| s.bg(rgb(0x3a3a3a)))
                                    })
                                    .child(opt.label)
                                    .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                                        entity.update(cx, |this, cx| {
                                            this.selected_speaker = Some(value.clone());
                                            this.speaker_dropdown_open = false;
                                            // Clear previous data when changing speaker
                                            this.cea2034_curves.clear();
                                            this.directivity_data = None;
                                            this.contour_data = None;
                                            this.data_load_state = LoadState::Idle;
                                            // Load versions for this speaker
                                            this.load_versions(cx);
                                        });
                                    })
                            })),
                    )
                    .with_priority(1),
                )
            })
    }

    fn render_version_dropdown(
        &mut self,
        options: Vec<SelectOption>,
        current: Option<String>,
        is_open: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let entity = cx.entity().clone();
        let entity_for_toggle = cx.entity().clone();

        div()
            .relative()
            .id("version-dropdown-container")
            .child(
                div()
                    .id("version-select")
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .min_w(px(120.0))
                    .bg(rgb(0x2a2a2a))
                    .border_1()
                    .border_color(rgb(0x3a3a3a))
                    .rounded_md()
                    .cursor_pointer()
                    .text_sm()
                    .hover(|s| s.border_color(rgb(0x007acc)))
                    .child(
                        div()
                            .text_color(if current.is_some() {
                                rgb(0xffffff)
                            } else {
                                rgb(0x666666)
                            })
                            .child(
                                current
                                    .clone()
                                    .unwrap_or_else(|| "Select version...".into()),
                            ),
                    )
                    .child(div().text_xs().text_color(rgb(0x666666)).child("▼"))
                    .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                        entity_for_toggle.update(cx, |this, cx| {
                            this.version_dropdown_open = !this.version_dropdown_open;
                            this.speaker_dropdown_open = false;
                            this.section_dropdown_open = false;
                            cx.notify();
                        });
                    }),
            )
            .when(is_open, |el| {
                el.child(
                    deferred(
                        div()
                            .id("version-dropdown")
                            .absolute()
                            .top_full()
                            .left_0()
                            .mt_1()
                            .w(px(150.0))
                            .max_h(px(300.0))
                            .overflow_y_scroll()
                            .bg(rgb(0x2a2a2a))
                            .border_1()
                            .border_color(rgb(0x3a3a3a))
                            .rounded_md()
                            .shadow_lg()
                            .py_1()
                            .children(options.into_iter().enumerate().map(|(i, opt)| {
                                let is_selected = current.as_ref() == Some(&opt.value.to_string());
                                let value = opt.value.to_string();
                                let entity = entity.clone();

                                div()
                                    .id(ElementId::NamedInteger("version-opt".into(), i as u64))
                                    .px_3()
                                    .py(px(6.0))
                                    .cursor_pointer()
                                    .text_sm()
                                    .when(is_selected, |el| {
                                        el.bg(rgb(0x007acc)).text_color(rgb(0xffffff))
                                    })
                                    .when(!is_selected, |el| {
                                        el.text_color(rgb(0xcccccc)).hover(|s| s.bg(rgb(0x3a3a3a)))
                                    })
                                    .child(opt.label)
                                    .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                                        entity.update(cx, |this, cx| {
                                            this.selected_version = Some(value.clone());
                                            this.version_dropdown_open = false;
                                            // Load speaker data with the selected version
                                            this.load_speaker_data(cx);
                                        });
                                    })
                            })),
                    )
                    .with_priority(1),
                )
            })
    }

    fn render_section_dropdown(
        &mut self,
        options: Vec<SelectOption>,
        current: &'static str,
        is_open: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let entity = cx.entity().clone();
        let entity_for_toggle = cx.entity().clone();

        div()
            .relative()
            .id("section-dropdown-container")
            .child(
                div()
                    .id("section-select")
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .min_w(px(180.0))
                    .bg(rgb(0x2a2a2a))
                    .border_1()
                    .border_color(rgb(0x3a3a3a))
                    .rounded_md()
                    .cursor_pointer()
                    .text_sm()
                    .hover(|s| s.border_color(rgb(0x007acc)))
                    .child(div().text_color(rgb(0xffffff)).child(current))
                    .child(div().text_xs().text_color(rgb(0x666666)).child("▼"))
                    .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                        entity_for_toggle.update(cx, |this, cx| {
                            this.section_dropdown_open = !this.section_dropdown_open;
                            this.speaker_dropdown_open = false;
                            this.version_dropdown_open = false;
                            cx.notify();
                        });
                    }),
            )
            .when(is_open, |el| {
                el.child(
                    deferred(
                        div()
                            .id("section-dropdown")
                            .absolute()
                            .top_full()
                            .left_0()
                            .mt_1()
                            .w(px(200.0))
                            .bg(rgb(0x2a2a2a))
                            .border_1()
                            .border_color(rgb(0x3a3a3a))
                            .rounded_md()
                            .shadow_lg()
                            .py_1()
                            .children(options.into_iter().enumerate().map(|(i, opt)| {
                                let is_selected = current == opt.value.as_ref();
                                let label_str = opt.label.to_string();
                                let entity = entity.clone();

                                div()
                                    .id(ElementId::NamedInteger("section-opt".into(), i as u64))
                                    .px_3()
                                    .py(px(6.0))
                                    .cursor_pointer()
                                    .text_sm()
                                    .when(is_selected, |el| {
                                        el.bg(rgb(0x007acc)).text_color(rgb(0xffffff))
                                    })
                                    .when(!is_selected, |el| {
                                        el.text_color(rgb(0xcccccc)).hover(|s| s.bg(rgb(0x3a3a3a)))
                                    })
                                    .child(opt.label)
                                    .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                                        let section = PlotSection::all()
                                            .into_iter()
                                            .find(|s| s.label() == label_str)
                                            .unwrap_or_default();
                                        entity.update(cx, |this, _| {
                                            this.current_section = section;
                                            this.section_dropdown_open = false;
                                        });
                                    })
                            })),
                    )
                    .with_priority(1),
                )
            })
    }

    fn render_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let content: Div = match self.data_load_state {
            LoadState::Idle => self.render_welcome(),
            LoadState::Loading => self.render_loading(),
            LoadState::Error(ref e) => self.render_error(e),
            LoadState::Loaded => match self.current_section {
                PlotSection::CEA2034 => self.render_cea2034_plot(),
                PlotSection::HorizontalSPL => self.render_directivity_plot("horizontal"),
                PlotSection::VerticalSPL => self.render_directivity_plot("vertical"),
                PlotSection::Contour => self.render_contour_plot(cx),
            },
        };

        div()
            .id("content-scroll")
            .flex_1()
            .h_full()
            .overflow_y_scroll()
            .bg(rgb(0xffffff))
            .p_8()
            .child(content)
            // Close dropdowns when clicking on content area
            .on_click(cx.listener(|this, _, _window, _cx| {
                this.speaker_dropdown_open = false;
                this.section_dropdown_open = false;
            }))
    }

    fn render_welcome(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .h_full()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0x333333))
                    .child("Spinorama Viewer"),
            )
            .child(
                div()
                    .text_base()
                    .text_color(rgb(0x666666))
                    .max_w(px(400.0))
                    .text_center()
                    .child("Select a speaker from the dropdown above to view its frequency response measurements from spinorama.org."),
            )
    }

    fn render_loading(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .h_full()
            .gap_4()
            .child(Spinner::new().size(SpinnerSize::Xl))
            .child(
                div()
                    .text_base()
                    .text_color(rgb(0x666666))
                    .child("Loading speaker data..."),
            )
    }

    fn render_error(&self, error: &str) -> Div {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .h_full()
            .gap_4()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xd32f2f))
                    .child("Error Loading Data"),
            )
            .child(
                div()
                    .text_base()
                    .text_color(rgb(0x666666))
                    .max_w(px(400.0))
                    .text_center()
                    .child(error.to_string()),
            )
    }

    fn render_cea2034_plot(&self) -> Div {
        let colors = cea2034_colors();

        let chart_width = 800.0;
        let chart_height = 400.0;

        // Separate DI curves from SPL curves
        let spl_curve_names = [
            "On Axis",
            "Listening Window",
            "Early Reflections",
            "Sound Power",
        ];
        let di_curve_names = ["Early Reflections DI", "Sound Power DI"];

        // Build PlotCurve list for SPL curves (primary axis)
        let mut plot_curves: Vec<PlotCurve> = spl_curve_names
            .iter()
            .filter_map(|&name| {
                let curve = self.cea2034_curves.get(name)?;
                let color = colors
                    .get(name)
                    .cloned()
                    .unwrap_or(D3Color::rgb(128, 128, 128));
                let points: Vec<LinePoint> = curve
                    .freq
                    .iter()
                    .zip(curve.spl.iter())
                    .filter(|(&f, _)| f >= 20.0 && f <= 20000.0)
                    .map(|(&f, &spl)| LinePoint::new(f, spl))
                    .collect();
                if points.is_empty() {
                    return None;
                }
                Some(PlotCurve::new(points, color))
            })
            .collect();

        // Add DI curves (secondary axis)
        let di_curves: Vec<PlotCurve> = di_curve_names
            .iter()
            .filter_map(|&name| {
                let curve = self.cea2034_curves.get(name)?;
                let color = colors
                    .get(name)
                    .cloned()
                    .unwrap_or(D3Color::rgb(128, 128, 128));
                let points: Vec<LinePoint> = curve
                    .freq
                    .iter()
                    .zip(curve.spl.iter())
                    .filter(|(&f, _)| f >= 20.0 && f <= 20000.0)
                    .map(|(&f, &spl)| LinePoint::new(f, spl))
                    .collect();
                if points.is_empty() {
                    return None;
                }
                Some(PlotCurve::new(points, color).secondary_axis())
            })
            .collect();
        plot_curves.extend(di_curves);

        // Configure secondary axis for DI curves
        // Note: Only include tick values up to 20 for labels (full domain is -5 to 45)
        let secondary_axis = Some(SecondaryAxisConfig {
            domain: (-5.0, 45.0),
            title: "DI (dB)",
            tick_values: vec![-5.0, 0.0, 5.0, 10.0, 15.0, 20.0], // Only show labels up to 20
        });

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0x333333))
                    .child(format!(
                        "CEA2034 - {}",
                        self.selected_speaker.as_deref().unwrap_or("Unknown")
                    )),
            )
            .child(render_freq_spl_plot(
                plot_curves,
                (-40.0, 10.0), // SPL domain
                secondary_axis,
                chart_width,
                chart_height,
            ))
            // Legend
            .child(self.render_legend(&colors))
    }

    fn render_legend(&self, colors: &HashMap<&'static str, D3Color>) -> Div {
        div()
            .flex()
            .flex_wrap()
            .gap_4()
            .p_4()
            .bg(rgb(0xf5f5f5))
            .rounded_md()
            .children(CEA2034_CURVES.iter().map(|&name| {
                let color = colors
                    .get(name)
                    .cloned()
                    .unwrap_or(D3Color::rgb(128, 128, 128));
                let (r, g, b) = (
                    (color.r * 255.0) as u32,
                    (color.g * 255.0) as u32,
                    (color.b * 255.0) as u32,
                );
                let font_config = VectorFontConfig::horizontal(12.0, hsla(0.0, 0.0, 0.2, 1.0));

                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(3.0))
                            .bg(rgb((r as u32) << 16 | (g as u32) << 8 | (b as u32))),
                    )
                    .child(render_vector_text(name, &font_config))
            }))
    }

    fn render_directivity_plot(&self, plane: &str) -> Div {
        // Create a viridis-like color palette for directivity
        let viridis_colors = vec![
            D3Color::from_hex(0x440154), // Dark purple
            D3Color::from_hex(0x414487), // Purple-blue
            D3Color::from_hex(0x2a788e), // Teal
            D3Color::from_hex(0x22a884), // Green-teal
            D3Color::from_hex(0x7ad151), // Light green
            D3Color::from_hex(0xfde725), // Yellow
        ];

        let Some(ref directivity) = self.directivity_data else {
            return div().flex().items_center().justify_center().h_full().child(
                div()
                    .text_base()
                    .text_color(rgb(0x666666))
                    .child("No directivity data available for this speaker."),
            );
        };

        let curves = if plane == "horizontal" {
            &directivity.horizontal
        } else {
            &directivity.vertical
        };

        if curves.is_empty() {
            return div().flex().items_center().justify_center().h_full().child(
                div()
                    .text_base()
                    .text_color(rgb(0x666666))
                    .child(format!("No {} directivity data available.", plane)),
            );
        }

        let chart_width = 800.0;
        let chart_height = 400.0;

        // Generate colors for different angles and build PlotCurve list
        let num_curves = curves.len();
        let plot_curves: Vec<PlotCurve> = curves
            .iter()
            .enumerate()
            .map(|(i, curve)| {
                let t = i as f32 / (num_curves.max(1) - 1).max(1) as f32;
                let color = d3rs::color::interpolate_colors(&viridis_colors, t);

                let points: Vec<LinePoint> = curve
                    .freq
                    .iter()
                    .zip(curve.spl.iter())
                    .filter(|(&f, _)| f >= 20.0 && f <= 20000.0)
                    .map(|(&f, &spl)| LinePoint::new(f, spl))
                    .collect();

                PlotCurve::new(points, color).stroke_width(1.5)
            })
            .collect();

        // Get angle range for legend
        let angle_min = curves.first().map(|c| c.angle).unwrap_or(-60.0);
        let angle_max = curves.last().map(|c| c.angle).unwrap_or(60.0);

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0x333333))
                    .child(format!(
                        "{} SPL - {}",
                        if plane == "horizontal" {
                            "Horizontal"
                        } else {
                            "Vertical"
                        },
                        self.selected_speaker.as_deref().unwrap_or("Unknown")
                    )),
            )
            .child(render_freq_spl_plot(
                plot_curves,
                (-40.0, 10.0), // SPL domain
                None,          // No secondary axis for directivity plots
                chart_width,
                chart_height,
            ))
            // Angle legend
            .child({
                let font_config = VectorFontConfig::horizontal(10.0, hsla(0.0, 0.0, 0.4, 1.0));

                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_4()
                    .bg(rgb(0xf5f5f5))
                    .rounded_md()
                    .child(render_vector_text(
                        &format!("{:.0}°", angle_min),
                        &font_config,
                    ))
                    // Simplified gradient legend (using color strip segments)
                    .children((0..6).map(|i| {
                        let color =
                            d3rs::color::interpolate_colors(&viridis_colors, i as f32 / 5.0);
                        let (r, g, b) = (
                            (color.r * 255.0) as u32,
                            (color.g * 255.0) as u32,
                            (color.b * 255.0) as u32,
                        );
                        div().flex_1().h(px(16.0)).bg(rgb((r << 16) | (g << 8) | b))
                    }))
                    .child(render_vector_text(
                        &format!("{:.0}°", angle_max),
                        &font_config,
                    ))
            })
    }

    /// Render a toggle button for switching between isoline and surface modes
    fn render_mode_toggle(
        &self,
        mode: ContourRenderMode,
        id: &'static str,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let entity = cx.entity().clone();

        div()
            .id(id)
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x666666))
                    .child("Render:"),
            )
            .child(
                div()
                    .id(ElementId::Name(format!("{}-btn", id).into()))
                    .flex()
                    .items_center()
                    .px_3()
                    .py_1()
                    .bg(rgb(0xe0e0e0))
                    .border_1()
                    .border_color(rgb(0xcccccc))
                    .rounded_md()
                    .cursor_pointer()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(0x333333))
                    .hover(|s| s.bg(rgb(0xd0d0d0)))
                    .child(mode.label())
                    .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                        entity.update(cx, |this, cx| {
                            on_click(this, cx);
                            cx.notify();
                        });
                    }),
            )
    }

    /// Render contour plot from SPL Horizontal Contour data (new format with full -180 to +180 range)
    fn render_contour_from_contour_data(&self, title: &str, render_mode: ContourRenderMode) -> Option<Div> {
        let theme = DefaultAxisTheme;

        let contour_data = self.contour_data.as_ref()?;

        let freq_count = contour_data.freq_count;
        let angle_count = contour_data.angle_count;

        if freq_count == 0 || angle_count == 0 {
            return None;
        }

        // Get actual angle range from data
        let angle_min = contour_data
            .angles
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min);
        let angle_max = contour_data
            .angles
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        // Get frequency range from data
        let freq_min = contour_data
            .freq
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min);
        let freq_max = contour_data
            .freq
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        println!(
            "Contour (SPL Horizontal Contour): {} angles x {} freqs, angle range: {:.1}° to {:.1}°, freq range: {:.1}Hz to {:.1}Hz",
            angle_count, freq_count, angle_min, angle_max, freq_min, freq_max
        );

        // Calculate SPL range
        let spl_min = contour_data
            .spl
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min);
        let spl_max = contour_data
            .spl
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        // Generate contour thresholds (every 3 dB based on actual data range)
        let threshold_min = ((spl_min / 3.0).floor() * 3.0) as i32;
        let threshold_max = ((spl_max / 3.0).ceil() * 3.0) as i32;
        let thresholds: Vec<f64> = (threshold_min..=threshold_max)
            .step_by(3)
            .map(|v| v as f64)
            .collect();

        // For the contour generator, we pass the log-transformed frequencies
        let log_freq_values: Vec<f64> = contour_data.freq.iter().map(|f| f.ln()).collect();

        // Fixed axis ranges based on data or reasonable defaults
        let log_freq_min = freq_min.max(20.0).ln();
        let log_freq_max = freq_max.min(20000.0).ln();

        // Create contour generator with explicit log-transformed x values
        let generator = ContourGenerator::new(freq_count, angle_count)
            .x_values(log_freq_values)
            .y_values(contour_data.angles.clone());

        let contours = generator.contours(&contour_data.spl, &thresholds);

        let chart_width = 800.0;
        let chart_height = 300.0;

        // Create scales with data-driven ranges
        let freq_scale = LinearScale::new()
            .domain(log_freq_min, log_freq_max)
            .range(0.0, chart_width as f64);

        let angle_scale = LinearScale::new()
            .domain(angle_min, angle_max)
            .range(0.0, chart_height as f64);

        // Configure rendering based on mode
        let is_surface = render_mode == ContourRenderMode::Surface;
        let contour_config = ContourConfig::new()
            .stroke_width(if is_surface { 0.5 } else { 1.5 })
            .fill(is_surface)
            .fill_opacity(if is_surface { 0.6 } else { 0.0 })
            .stroke_opacity(if is_surface { 0.8 } else { 1.0 })
            .color_scale(move |t| viridis_color_scale()(t));

        // Build frequency tick values in log space
        let freq_ticks: Vec<f64> = [20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0]
            .iter()
            .filter(|&&f| f >= freq_min && f <= freq_max)
            .map(|f| f.ln())
            .collect();

        Some(
            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0x333333))
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .child(render_axis(
                                    &angle_scale,
                                    &AxisConfig::left()
                                        .with_ticks(13)
                                        .with_formatter(|v| format!("{:.0}°", v))
                                        .with_title("Angle"),
                                    chart_height,
                                    &theme,
                                ))
                                .child(
                                    div()
                                        .w(px(chart_width as f32))
                                        .h(px(chart_height as f32))
                                        .relative()
                                        .bg(rgb(0xf8f8f8))
                                        .child(render_grid(
                                            &freq_scale,
                                            &angle_scale,
                                            &GridConfig::with_lines()
                                                .with_vertical_values(freq_ticks.clone()),
                                            chart_width,
                                            chart_height,
                                            &theme,
                                        ))
                                        .child(
                                            render_contour(
                                                contours,
                                                &freq_scale,
                                                &angle_scale,
                                                &contour_config,
                                            )
                                            .value_range(spl_min, spl_max)
                                            .height(px(chart_height as f32)),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .child(div().w(px(80.0)))
                                .child(render_axis(
                                    &freq_scale,
                                    &AxisConfig::bottom()
                                        .with_tick_values(freq_ticks)
                                        .with_formatter(|log_f| {
                                            let f = log_f.exp();
                                            if f >= 1000.0 {
                                                format!("{:.0}k", f / 1000.0)
                                            } else {
                                                format!("{:.0}", f)
                                            }
                                        })
                                        .with_title("Frequency (Hz)"),
                                    chart_width,
                                    &theme,
                                )),
                        ),
                )
                // Color legend
                .child({
                    let font_config = VectorFontConfig::horizontal(10.0, hsla(0.0, 0.0, 0.4, 1.0));
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .p_2()
                        .bg(rgb(0xf5f5f5))
                        .rounded_md()
                        .child(render_vector_text(&format!("{:.0} dB", spl_min), &font_config))
                        .children((0..15).map(|i| {
                            let t = i as f64 / 14.0;
                            let color = viridis_color_scale()(t);
                            let (r, g, b) = (
                                (color.r * 255.0) as u32,
                                (color.g * 255.0) as u32,
                                (color.b * 255.0) as u32,
                            );
                            div().w(px(15.0)).h(px(15.0)).bg(rgb((r << 16) | (g << 8) | b))
                        }))
                        .child(render_vector_text(&format!("{:.0} dB", spl_max), &font_config))
                }),
        )
    }

    /// Render contour plot from directivity data (old format, typically -60 to +60 range)
    fn render_contour_from_directivity(&self, title: &str, render_mode: ContourRenderMode) -> Option<Div> {
        let theme = DefaultAxisTheme;

        let directivity = self.directivity_data.as_ref()?;
        let curves = &directivity.horizontal;

        if curves.is_empty() {
            return None;
        }

        // Get frequency points from first curve (assume all curves have same freq points)
        let all_freq_points = &curves[0].freq;

        // Filter frequencies to >= 100Hz
        let freq_start_idx = all_freq_points
            .iter()
            .position(|&f| f >= 100.0)
            .unwrap_or(0);
        let freq_points: Vec<f64> = all_freq_points
            .iter()
            .skip(freq_start_idx)
            .copied()
            .collect();
        let freq_count = freq_points.len();

        // Get angles from curves
        let angles: Vec<f64> = curves.iter().map(|c| c.angle).collect();
        let angle_count = angles.len();

        let angle_min = angles.iter().cloned().fold(f64::INFINITY, f64::min);
        let angle_max = angles.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        println!(
            "Contour (Directivity): {} curves, angle range: {:.1}° to {:.1}°, {} freq points",
            angle_count, angle_min, angle_max, freq_count
        );

        if freq_count == 0 || angle_count == 0 {
            return None;
        }

        // Create grid values (angle x frequency), filtered to >= 100Hz
        let mut grid_values: Vec<f64> = Vec::with_capacity(angle_count * freq_count);
        let mut spl_min = f64::INFINITY;
        let mut spl_max = f64::NEG_INFINITY;

        for curve in curves.iter() {
            for &spl in curve.spl.iter().skip(freq_start_idx) {
                grid_values.push(spl);
                if spl < spl_min {
                    spl_min = spl;
                }
                if spl > spl_max {
                    spl_max = spl;
                }
            }
        }

        // Generate contour thresholds (every 3 dB)
        let threshold_min = ((spl_min / 3.0).floor() * 3.0) as i32;
        let threshold_max = ((spl_max / 3.0).ceil() * 3.0) as i32;
        let thresholds: Vec<f64> = (threshold_min..=threshold_max)
            .step_by(3)
            .map(|v| v as f64)
            .collect();

        let log_freq_values: Vec<f64> = freq_points.iter().map(|f| f.ln()).collect();
        let log_freq_min = 100.0_f64.ln();
        let log_freq_max = 20000.0_f64.ln();

        let generator = ContourGenerator::new(freq_count, angle_count)
            .x_values(log_freq_values)
            .y_values(angles);

        let contours = generator.contours(&grid_values, &thresholds);

        let chart_width = 800.0;
        let chart_height = 300.0;

        let freq_scale = LinearScale::new()
            .domain(log_freq_min, log_freq_max)
            .range(0.0, chart_width as f64);

        let angle_scale = LinearScale::new()
            .domain(angle_min, angle_max)
            .range(0.0, chart_height as f64);

        // Configure rendering based on mode
        let is_surface = render_mode == ContourRenderMode::Surface;
        let contour_config = ContourConfig::new()
            .stroke_width(if is_surface { 0.5 } else { 1.5 })
            .fill(is_surface)
            .fill_opacity(if is_surface { 0.6 } else { 0.0 })
            .stroke_opacity(if is_surface { 0.8 } else { 1.0 })
            .color_scale(move |t| viridis_color_scale()(t));

        let freq_ticks: Vec<f64> = vec![
            100.0_f64.ln(),
            200.0_f64.ln(),
            500.0_f64.ln(),
            1000.0_f64.ln(),
            2000.0_f64.ln(),
            5000.0_f64.ln(),
            10000.0_f64.ln(),
            20000.0_f64.ln(),
        ];

        Some(
            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0x333333))
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .child(render_axis(
                                    &angle_scale,
                                    &AxisConfig::left()
                                        .with_ticks(9)
                                        .with_formatter(|v| format!("{:.0}°", v))
                                        .with_title("Angle"),
                                    chart_height,
                                    &theme,
                                ))
                                .child(
                                    div()
                                        .w(px(chart_width as f32))
                                        .h(px(chart_height as f32))
                                        .relative()
                                        .bg(rgb(0xf8f8f8))
                                        .child(render_grid(
                                            &freq_scale,
                                            &angle_scale,
                                            &GridConfig::with_lines()
                                                .with_vertical_values(freq_ticks.clone()),
                                            chart_width,
                                            chart_height,
                                            &theme,
                                        ))
                                        .child(
                                            render_contour(
                                                contours,
                                                &freq_scale,
                                                &angle_scale,
                                                &contour_config,
                                            )
                                            .value_range(spl_min, spl_max)
                                            .height(px(chart_height as f32)),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .child(div().w(px(80.0)))
                                .child(render_axis(
                                    &freq_scale,
                                    &AxisConfig::bottom()
                                        .with_tick_values(freq_ticks)
                                        .with_formatter(|log_f| {
                                            let f = log_f.exp();
                                            if f >= 1000.0 {
                                                format!("{:.0}k", f / 1000.0)
                                            } else {
                                                format!("{:.0}", f)
                                            }
                                        })
                                        .with_title("Frequency (Hz)"),
                                    chart_width,
                                    &theme,
                                )),
                        ),
                )
                // Color legend
                .child({
                    let font_config = VectorFontConfig::horizontal(10.0, hsla(0.0, 0.0, 0.4, 1.0));
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .p_2()
                        .bg(rgb(0xf5f5f5))
                        .rounded_md()
                        .child(render_vector_text(&format!("{:.0} dB", spl_min), &font_config))
                        .children((0..15).map(|i| {
                            let t = i as f64 / 14.0;
                            let color = viridis_color_scale()(t);
                            let (r, g, b) = (
                                (color.r * 255.0) as u32,
                                (color.g * 255.0) as u32,
                                (color.b * 255.0) as u32,
                            );
                            div().w(px(15.0)).h(px(15.0)).bg(rgb((r << 16) | (g << 8) | b))
                        }))
                        .child(render_vector_text(&format!("{:.0} dB", spl_max), &font_config))
                }),
        )
    }

    fn render_contour_plot(&mut self, cx: &mut Context<Self>) -> Div {
        let has_contour_data = self.contour_data.is_some();
        let has_directivity_data = self.directivity_data.as_ref().map_or(false, |d| !d.horizontal.is_empty());

        if !has_contour_data && !has_directivity_data {
            return div().flex().items_center().justify_center().h_full().child(
                div()
                    .text_base()
                    .text_color(rgb(0x666666))
                    .child("No contour data available for this speaker."),
            );
        }

        let speaker_name = self.selected_speaker.as_deref().unwrap_or("Unknown");
        let spl_mode = self.contour_mode_spl;
        let directivity_mode = self.contour_mode_directivity;

        // Render toggle buttons with the contour plots
        let spl_toggle = self.render_mode_toggle(
            spl_mode,
            "spl-contour-toggle",
            |app, _cx| {
                app.contour_mode_spl = app.contour_mode_spl.toggle();
            },
            cx,
        );

        let directivity_toggle = self.render_mode_toggle(
            directivity_mode,
            "directivity-contour-toggle",
            |app, _cx| {
                app.contour_mode_directivity = app.contour_mode_directivity.toggle();
            },
            cx,
        );

        // Pre-render the contour plots
        let spl_contour = self.render_contour_from_contour_data("SPL Horizontal Contour (Full 360°)", spl_mode);
        let directivity_contour = self.render_contour_from_directivity("Directivity Contour (SPL Horizontal)", directivity_mode);

        div()
            .flex()
            .flex_col()
            .gap_8()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0x333333))
                    .child(format!("Horizontal Contour Plots - {}", speaker_name)),
            )
            // SPL Horizontal Contour (new format, -180 to +180) with toggle
            .when_some(spl_contour, |el, contour_div| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(spl_toggle)
                        .child(contour_div)
                )
            })
            // Directivity-based contour (old format, typically -60 to +60) with toggle
            .when_some(directivity_contour, |el, contour_div| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(directivity_toggle)
                        .child(contour_div)
                )
            })
    }
}

impl Render for SpinoramaApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("main-container")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xffffff))
            .child(self.render_header(cx))
            .child(self.render_content(cx))
    }
}

// Define actions
actions!(spinorama_demo, [Quit]);

fn main() {
    Application::new().run(|cx| {
        // Activate app and register quit action
        cx.activate(true);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);

        // Set up application menu
        cx.set_menus(vec![Menu {
            name: "Spinorama Viewer".into(),
            items: vec![
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("Quit Spinorama Viewer", Quit),
            ],
        }]);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(100.0), px(100.0)),
                    size: size(px(1200.0), px(800.0)),
                })),
                titlebar: Some(TitlebarOptions {
                    title: Some("Spinorama Viewer".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_window, cx| cx.new(SpinoramaApp::new),
        )
        .expect("Failed to open window");
    });
}
