use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use autoeq::read::{
    extract_cea2034_curves_original, fetch_available_speakers, fetch_contour_data,
    fetch_directivity_data, fetch_measurement_plot_data, ContourPlotData,
};
use autoeq::{Curve, DirectivityData};
use d3rs::axis::{render_axis, AxisConfig, DefaultAxisTheme};
use d3rs::brush::{BrushSelection, BrushState};
use d3rs::color::D3Color;
use d3rs::contour::ContourGenerator;
use d3rs::grid::{render_grid, GridConfig};
use d3rs::prelude::{LinearScale, LogScale};
use d3rs::shape::contour::{
    render_contour, render_contour_bands, render_heatmap, ContourConfig, HeatmapData,
};
// Radial shape functions could be used in future - currently using canvas-based custom rendering
// use d3rs::shape::radial::{polar_grid_circles, polar_grid_rays, radial_line, RadialLineConfig, RadialPoint};
use d3rs::surface::{render_surface, ColorScaleType, SurfaceConfig, SurfaceData};
use d3rs::text::{render_vector_text, VectorFontConfig};
use d3rs::zoom::ZoomState;
use gpui::prelude::*;
use gpui::{deferred, *};
use gpui_ui_kit::{SelectOption, Spinner, SpinnerSize};
use tokio::runtime::Runtime;
use urlencoding;

use super::render::render_freq_spl_plot;
use super::types::{
    BrushOverlay, ChartId, Colormap, ContourRenderMode, DirectivityPlane, LinePoint, LoadState,
    PlotCurve, PlotSection, SecondaryAxisConfig, SurfaceProjection,
};
use super::utils::{
    cea2034_colors, format_frequency, get_angle_range, interpolate_spl_at_frequency, CEA2034_CURVES,
};

/// Main application state
pub struct SpinoramaApp {
    pub runtime: Arc<Runtime>,
    // Speaker list
    pub speakers: Vec<String>,
    pub speakers_load_state: LoadState,
    // Version list for selected speaker
    pub versions: Vec<String>,
    pub versions_load_state: LoadState,
    // Selection state
    pub selected_speaker: Option<String>,
    pub selected_version: Option<String>,
    pub selected_measurement: String,
    // Data state
    pub cea2034_curves: HashMap<String, Curve>,
    pub directivity_data: Option<DirectivityData>,
    pub contour_data: Option<ContourPlotData>,
    pub data_load_state: LoadState,
    // UI state
    pub current_section: PlotSection,
    pub speaker_dropdown_open: bool,
    pub version_dropdown_open: bool,
    pub section_dropdown_open: bool,
    // Contour render mode for each plot (SPL Horizontal Contour, Directivity Contour)
    pub contour_mode_spl: ContourRenderMode,
    pub contour_mode_directivity: ContourRenderMode,
    // Colormap selection for contour plots
    pub contour_colormap: Colormap,
    // Zoom state for frequency/SPL plots (CEA2034, horizontal/vertical SPL)
    pub freq_spl_zoom: ZoomState,
    pub freq_spl_brush: BrushState,
    // Zoom state for SPL contour plot
    pub spl_contour_zoom: ZoomState,
    pub spl_contour_brush: BrushState,
    // Zoom state for directivity contour plot
    pub directivity_contour_zoom: ZoomState,
    pub directivity_contour_brush: BrushState,
    // Track which chart is currently being brushed (for event handling)
    pub active_brush_chart: Option<ChartId>,
    // Chart bounds for mouse position calculation (window-relative)
    // These are shared via Rc<RefCell> to allow capture in closures
    pub freq_spl_chart_bounds: Rc<RefCell<Option<Bounds<Pixels>>>>,
    pub spl_contour_chart_bounds: Rc<RefCell<Option<Bounds<Pixels>>>>,
    pub directivity_contour_chart_bounds: Rc<RefCell<Option<Bounds<Pixels>>>>,

    // Polar directivity plot state
    pub polar_selected_frequencies: Vec<f64>,
    pub polar_plane: DirectivityPlane,

    // 3D surface plot state
    pub surface_projection: SurfaceProjection,
    pub surface_rotation_azimuth: f32,
    pub surface_rotation_elevation: f32,
    pub surface_wireframe: bool,

    // Polar contour plot state
    pub polar_contour_freq_range: (f64, f64),
}
impl SpinoramaApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
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
            contour_colormap: Colormap::default(),
            // Zoom for freq/SPL plots: X=20Hz-20kHz (log), Y=-40 to 10 dB (linear)
            freq_spl_zoom: ZoomState::new(20.0, 20000.0, -40.0, 10.0).with_log_x(true),
            freq_spl_brush: BrushState::new(),
            // Zoom for SPL contour: X=100Hz-20kHz (log), Y=-180 to 180 deg (linear)
            spl_contour_zoom: ZoomState::new(100.0, 20000.0, -180.0, 180.0).with_log_x(true),
            spl_contour_brush: BrushState::new(),
            // Zoom for directivity contour: same ranges
            directivity_contour_zoom: ZoomState::new(100.0, 20000.0, -180.0, 180.0)
                .with_log_x(true),
            directivity_contour_brush: BrushState::new(),
            active_brush_chart: None,
            // Initialize chart bounds as None - will be captured during render
            freq_spl_chart_bounds: Rc::new(RefCell::new(None)),
            spl_contour_chart_bounds: Rc::new(RefCell::new(None)),
            directivity_contour_chart_bounds: Rc::new(RefCell::new(None)),

            // Polar directivity plot state - default frequencies at octaves
            polar_selected_frequencies: vec![100.0, 1000.0, 10000.0],
            polar_plane: DirectivityPlane::default(),

            // 3D surface plot state
            surface_projection: SurfaceProjection::default(),
            surface_rotation_azimuth: 45.0,
            surface_rotation_elevation: 30.0,
            surface_wireframe: false,

            // Polar contour frequency range (20Hz - 20kHz)
            polar_contour_freq_range: (20.0, 20000.0),
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
                    async move {
                        fetch_contour_data(&speaker, &version, "horizontal")
                            .await
                            .ok()
                    }
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

    pub fn render_header(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
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

    pub fn render_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let content: Div = match self.data_load_state {
            LoadState::Idle => self.render_welcome(),
            LoadState::Loading => self.render_loading(),
            LoadState::Error(ref e) => self.render_error(e),
            LoadState::Loaded => match self.current_section {
                PlotSection::CEA2034 => self.render_cea2034_plot(cx),
                PlotSection::HorizontalSPL => self.render_directivity_plot("horizontal", cx),
                PlotSection::VerticalSPL => self.render_directivity_plot("vertical", cx),
                PlotSection::Contour => self.render_contour_plot(cx),
                PlotSection::PolarDirectivity => self.render_polar_directivity_plot(cx),
                PlotSection::Surface3D => self.render_surface_3d_plot(cx),
                PlotSection::PolarContour => self.render_polar_contour_plot(cx),
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

    /// Wrap a chart in an interactive container with brush/zoom handlers
    fn wrap_freq_spl_chart_interactive(
        &self,
        chart: Div,
        chart_id: ChartId,
        chart_width: f32,
        chart_height: f32,
        cx: &mut Context<Self>,
    ) -> Div {
        let entity = cx.entity().clone();
        let entity2 = entity.clone();
        let entity3 = entity.clone();
        let entity4 = entity.clone();

        // Get the chart bounds Rc for this chart type
        let chart_bounds = match chart_id {
            ChartId::FreqSpl => self.freq_spl_chart_bounds.clone(),
            ChartId::SplContour => self.spl_contour_chart_bounds.clone(),
            ChartId::DirectivityContour => self.directivity_contour_chart_bounds.clone(),
        };
        let chart_bounds_for_move = chart_bounds.clone();
        let chart_bounds_for_up = chart_bounds.clone();
        let chart_bounds_for_prepaint = chart_bounds.clone();

        // Left axis spacer width (from render_freq_spl_plot)
        let left_margin = 80.0_f32;

        // Helper to convert window position to chart-relative coordinates
        // using the stored chart bounds
        fn to_chart_coords(
            pos: Point<Pixels>,
            bounds: &Option<Bounds<Pixels>>,
            left_margin: f32,
            chart_width: f32,
            chart_height: f32,
        ) -> (f32, f32) {
            if let Some(b) = bounds {
                // Subtract wrapper origin to get element-relative coordinates
                let rel_x = f32::from(pos.x) - f32::from(b.origin.x) - left_margin;
                let rel_y = f32::from(pos.y) - f32::from(b.origin.y);
                (
                    rel_x.max(0.0).min(chart_width),
                    rel_y.max(0.0).min(chart_height),
                )
            } else {
                // Fallback: no bounds captured yet, use raw position with just X margin
                let chart_x = (f32::from(pos.x) - left_margin).max(0.0).min(chart_width);
                let chart_y = f32::from(pos.y).max(0.0).min(chart_height);
                (chart_x, chart_y)
            }
        }

        // Outer wrapper that captures bounds via on_children_prepainted
        div()
            .on_children_prepainted(move |children_bounds, _window, _cx| {
                // The first child is our inner interactive div - store its bounds
                if let Some(inner_bounds) = children_bounds.first() {
                    *chart_bounds_for_prepaint.borrow_mut() = Some(*inner_bounds);
                }
            })
            .child(
                // Inner div with mouse handlers and id
                div()
                    .id(match chart_id {
                        ChartId::FreqSpl => "freq-spl-chart",
                        ChartId::SplContour => "spl-contour-chart",
                        ChartId::DirectivityContour => "directivity-contour-chart",
                    })
                    .relative()
                    .child(chart)
                    // Mouse down to start brush
                    .on_mouse_down(MouseButton::Left, move |event, _window, cx| {
                        let pos = event.position;
                        let bounds = chart_bounds.borrow();
                        let (chart_x, chart_y) =
                            to_chart_coords(pos, &bounds, left_margin, chart_width, chart_height);

                        entity.update(cx, |this, cx| {
                            this.active_brush_chart = Some(chart_id);
                            match chart_id {
                                ChartId::FreqSpl => {
                                    this.freq_spl_brush.start(chart_x as f64, chart_y as f64)
                                }
                                ChartId::SplContour => {
                                    this.spl_contour_brush.start(chart_x as f64, chart_y as f64)
                                }
                                ChartId::DirectivityContour => this
                                    .directivity_contour_brush
                                    .start(chart_x as f64, chart_y as f64),
                            }
                            cx.notify();
                        });
                    })
                    // Mouse move to update brush during drag
                    .on_mouse_move(move |event, _window, cx| {
                        entity2.update(cx, |this, cx| {
                            if this.active_brush_chart == Some(chart_id) {
                                let pos = event.position;
                                let bounds = chart_bounds_for_move.borrow();
                                let (chart_x, chart_y) = to_chart_coords(
                                    pos,
                                    &bounds,
                                    left_margin,
                                    chart_width,
                                    chart_height,
                                );

                                match chart_id {
                                    ChartId::FreqSpl => {
                                        this.freq_spl_brush.update(chart_x as f64, chart_y as f64)
                                    }
                                    ChartId::SplContour => this
                                        .spl_contour_brush
                                        .update(chart_x as f64, chart_y as f64),
                                    ChartId::DirectivityContour => this
                                        .directivity_contour_brush
                                        .update(chart_x as f64, chart_y as f64),
                                }
                                cx.notify();
                            }
                        });
                    })
                    // Mouse up to end brush and apply zoom
                    .on_mouse_up(MouseButton::Left, move |event, _window, cx| {
                        entity3.update(cx, |this, cx| {
                            if this.active_brush_chart == Some(chart_id) {
                                let pos = event.position;
                                let bounds = chart_bounds_for_up.borrow();
                                let (chart_x, chart_y) = to_chart_coords(
                                    pos,
                                    &bounds,
                                    left_margin,
                                    chart_width,
                                    chart_height,
                                );

                                // Final update
                                match chart_id {
                                    ChartId::FreqSpl => {
                                        this.freq_spl_brush.update(chart_x as f64, chart_y as f64)
                                    }
                                    ChartId::SplContour => this
                                        .spl_contour_brush
                                        .update(chart_x as f64, chart_y as f64),
                                    ChartId::DirectivityContour => this
                                        .directivity_contour_brush
                                        .update(chart_x as f64, chart_y as f64),
                                }

                                // Get selection and apply zoom
                                let selection = match chart_id {
                                    ChartId::FreqSpl => this.freq_spl_brush.end(),
                                    ChartId::SplContour => this.spl_contour_brush.end(),
                                    ChartId::DirectivityContour => {
                                        this.directivity_contour_brush.end()
                                    }
                                };

                                if let Some(sel) = selection {
                                    // Check if selection is large enough
                                    if sel.width() >= 5.0 && sel.height() >= 5.0 {
                                        // Convert pixel selection to domain coordinates
                                        // using the current zoom state and chart dimensions
                                        this.apply_zoom_from_selection(
                                            chart_id,
                                            sel,
                                            chart_width,
                                            chart_height,
                                        );
                                    }
                                }

                                this.active_brush_chart = None;
                                cx.notify();
                            }
                        });
                    })
                    // Double click to reset zoom
                    .on_click(move |event, _window, cx| {
                        // Check for double-click (click_count >= 2)
                        if event.click_count() >= 2 {
                            entity4.update(cx, |this, cx| {
                                match chart_id {
                                    ChartId::FreqSpl => this.freq_spl_zoom.reset(),
                                    ChartId::SplContour => this.spl_contour_zoom.reset(),
                                    ChartId::DirectivityContour => {
                                        this.directivity_contour_zoom.reset()
                                    }
                                }
                                cx.notify();
                            });
                        }
                    }),
            )
    }

    /// Convert pixel selection to domain coordinates and apply zoom
    fn apply_zoom_from_selection(
        &mut self,
        chart_id: ChartId,
        sel: BrushSelection,
        chart_width: f32,
        chart_height: f32,
    ) {
        let (zoom_state, is_log_x) = match chart_id {
            ChartId::FreqSpl => (&mut self.freq_spl_zoom, true),
            ChartId::SplContour => (&mut self.spl_contour_zoom, true),
            ChartId::DirectivityContour => (&mut self.directivity_contour_zoom, true),
        };

        // Get current domain for scale inversion
        let x_domain = zoom_state.x_domain();
        let y_domain = zoom_state.y_domain();

        // Y-axis uses inverted range: pixel 0 = top = domain max, pixel height = bottom = domain min
        // This matches screen coordinates where Y increases downward but domain values increase upward
        let y_scale = LinearScale::new()
            .domain(y_domain.0, y_domain.1)
            .range(chart_height as f64, 0.0);

        // Convert pixel coordinates to domain
        // The brush module now does direct inversion without Y-swap
        let (x0, x1, y0, y1) = if is_log_x {
            let x_scale = LogScale::new()
                .domain(x_domain.0, x_domain.1)
                .range(0.0, chart_width as f64);
            let domain_sel = sel.to_domain(&x_scale, &y_scale);
            (domain_sel.x0, domain_sel.x1, domain_sel.y0, domain_sel.y1)
        } else {
            let x_scale = LinearScale::new()
                .domain(x_domain.0, x_domain.1)
                .range(0.0, chart_width as f64);
            let domain_sel = sel.to_domain(&x_scale, &y_scale);
            (domain_sel.x0, domain_sel.x1, domain_sel.y0, domain_sel.y1)
        };

        // Ensure x0 < x1 and y0 < y1 for zoom_to
        let (x_min, x_max) = if x0 < x1 { (x0, x1) } else { (x1, x0) };
        let (y_min, y_max) = if y0 < y1 { (y0, y1) } else { (y1, y0) };

        // Apply zoom
        zoom_state.zoom_to(x_min, x_max, y_min, y_max);
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

    fn render_cea2034_plot(&mut self, cx: &mut Context<Self>) -> Div {
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

        // Create the chart
        let chart = render_freq_spl_plot(
            plot_curves,
            self.freq_spl_zoom.x_domain(), // Frequency domain (zoomed)
            self.freq_spl_zoom.y_domain(), // SPL domain (zoomed)
            secondary_axis,
            chart_width,
            chart_height,
            self.freq_spl_brush
                .current_selection()
                .map(|sel| BrushOverlay { selection: sel }),
        );

        // Wrap with interactive handlers
        let interactive_chart = self.wrap_freq_spl_chart_interactive(
            chart,
            ChartId::FreqSpl,
            chart_width,
            chart_height,
            cx,
        );

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
            .child(interactive_chart)
            // Zoom status indicator
            .when(self.freq_spl_zoom.is_zoomed(), |el| {
                el.child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x666666))
                        .child("Zoomed (double-click to reset)"),
                )
            })
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

    fn render_directivity_plot(&mut self, plane: &str, _cx: &mut Context<Self>) -> Div {
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

        // Create the chart
        let chart = render_freq_spl_plot(
            plot_curves,
            self.freq_spl_zoom.x_domain(),
            self.freq_spl_zoom.y_domain(),
            None, // No secondary axis for directivity plots
            chart_width,
            chart_height,
            self.freq_spl_brush
                .current_selection()
                .map(|sel| BrushOverlay { selection: sel }),
        );

        // Wrap with interactive handlers
        let interactive_chart = self.wrap_freq_spl_chart_interactive(
            chart,
            ChartId::FreqSpl,
            chart_width,
            chart_height,
            _cx,
        );

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
            .child(interactive_chart)
            // Zoom status indicator
            .when(self.freq_spl_zoom.is_zoomed(), |el| {
                el.child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x666666))
                        .child("Zoomed (double-click to reset)"),
                )
            })
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
        let entity_for_colormap = cx.entity().clone();
        let colormap = self.contour_colormap;

        div()
            .id(id)
            .flex()
            .items_center()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_sm().text_color(rgb(0x666666)).child("Render:"))
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
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_sm().text_color(rgb(0x666666)).child("Colormap:"))
                    .child(
                        div()
                            .id(ElementId::Name(format!("{}-colormap-btn", id).into()))
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
                            .child(colormap.label())
                            .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                                entity_for_colormap.update(cx, |this, cx| {
                                    this.contour_colormap = this.contour_colormap.next();
                                    cx.notify();
                                });
                            }),
                    ),
            )
    }

    /// Render contour plot from SPL Horizontal Contour data (new format with full -180 to +180 range)
    fn render_contour_from_contour_data(
        &self,
        title: &str,
        render_mode: ContourRenderMode,
        colormap: Colormap,
    ) -> Option<Div> {
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

        // Generate contours and heatmap data based on render mode
        let is_isoline = render_mode == ContourRenderMode::Isoline;
        let is_surface = render_mode == ContourRenderMode::Surface;
        let is_heatmap = render_mode == ContourRenderMode::Heatmap;
        // Generate contours for Isoline mode
        let contours = if is_isoline {
            Some(generator.contours(&contour_data.spl, &thresholds))
        } else {
            None
        };
        // Generate contour bands for Surface mode (filled polygons between levels)
        let contour_bands = if is_surface {
            Some(generator.contour_bands(&contour_data.spl, &thresholds))
        } else {
            None
        };
        // For heatmap mode, use HeatmapData (renders using quads without anti-aliasing gaps)
        let heatmap_data = if is_heatmap {
            // Use log-transformed frequencies for the heatmap x-values
            let log_freq_values: Vec<f64> = contour_data.freq.iter().map(|f| f.ln()).collect();
            Some(HeatmapData::new(
                log_freq_values,
                contour_data.angles.clone(),
                contour_data.spl.clone(),
            ))
        } else {
            None
        };

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
        let color_scale = colormap.color_scale();
        let contour_config = ContourConfig::new()
            .stroke_width(if is_isoline { 1.5 } else { 0.5 })
            .fill(is_surface)
            .fill_opacity(if is_surface {
                0.6
            } else if is_heatmap {
                1.0
            } else {
                0.0
            })
            .stroke_opacity(if is_isoline { 1.0 } else { 0.8 })
            .color_scale(color_scale);

        // Build frequency tick values in log space
        let freq_ticks: Vec<f64> = [
            20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
        ]
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
                                        // In Isoline mode, render grid first (underneath lines)
                                        .when(is_isoline, |el| {
                                            el.child(render_grid(
                                                &freq_scale,
                                                &angle_scale,
                                                &GridConfig::with_lines()
                                                    .with_vertical_values(freq_ticks.clone()),
                                                chart_width,
                                                chart_height,
                                                &theme,
                                            ))
                                        })
                                        // Render contour bands (for Surface mode) - filled polygons
                                        .when_some(contour_bands.clone(), |el, bands| {
                                            el.child(
                                                render_contour_bands(
                                                    bands,
                                                    &freq_scale,
                                                    &angle_scale,
                                                    &contour_config,
                                                )
                                                .value_range(spl_min, spl_max)
                                                .height(px(chart_height as f32)),
                                            )
                                        })
                                        // Render heatmap (for Heatmap mode) - uses quads, no anti-aliasing gaps
                                        .when_some(heatmap_data.clone(), |el, data| {
                                            el.child(
                                                render_heatmap(
                                                    data,
                                                    &freq_scale,
                                                    &angle_scale,
                                                    &contour_config,
                                                )
                                                .value_range(spl_min, spl_max)
                                                .height(px(chart_height as f32)),
                                            )
                                        })
                                        // In Surface/Heatmap mode, render grid on top
                                        .when(is_surface || is_heatmap, |el| {
                                            el.child(render_grid(
                                                &freq_scale,
                                                &angle_scale,
                                                &GridConfig::with_lines()
                                                    .with_vertical_values(freq_ticks.clone()),
                                                chart_width,
                                                chart_height,
                                                &theme,
                                            ))
                                        })
                                        // Render contour lines (for Isoline mode)
                                        .when_some(contours.clone(), |el, c| {
                                            el.child(
                                                render_contour(
                                                    c,
                                                    &freq_scale,
                                                    &angle_scale,
                                                    &contour_config,
                                                )
                                                .value_range(spl_min, spl_max)
                                                .height(px(chart_height as f32)),
                                            )
                                        }),
                                ),
                        )
                        .child(
                            div().flex().child(div().w(px(80.0))).child(render_axis(
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
                        .child(render_vector_text(
                            &format!("{:.0} dB", spl_min),
                            &font_config,
                        ))
                        .children((0..15).map(|i| {
                            let t = i as f64 / 14.0;
                            let color = colormap.color_scale()(t);
                            let (r, g, b) = (
                                (color.r * 255.0) as u32,
                                (color.g * 255.0) as u32,
                                (color.b * 255.0) as u32,
                            );
                            div()
                                .w(px(15.0))
                                .h(px(15.0))
                                .bg(rgb((r << 16) | (g << 8) | b))
                        }))
                        .child(render_vector_text(
                            &format!("{:.0} dB", spl_max),
                            &font_config,
                        ))
                }),
        )
    }

    /// Render contour plot from directivity data (old format, typically -60 to +60 range)
    fn render_contour_from_directivity(
        &self,
        title: &str,
        render_mode: ContourRenderMode,
        colormap: Colormap,
    ) -> Option<Div> {
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
            .x_values(log_freq_values.clone())
            .y_values(angles.clone());

        // Generate contours and heatmap data based on render mode
        let is_isoline = render_mode == ContourRenderMode::Isoline;
        let is_surface = render_mode == ContourRenderMode::Surface;
        let is_heatmap = render_mode == ContourRenderMode::Heatmap;
        // Generate contours for Isoline mode
        let contours = if is_isoline {
            Some(generator.contours(&grid_values, &thresholds))
        } else {
            None
        };
        // Generate contour bands for Surface mode (filled polygons between levels)
        let contour_bands = if is_surface {
            Some(generator.contour_bands(&grid_values, &thresholds))
        } else {
            None
        };
        // For heatmap mode, use HeatmapData (renders using quads without anti-aliasing gaps)
        let heatmap_data = if is_heatmap {
            Some(HeatmapData::new(
                log_freq_values.clone(),
                angles.clone(),
                grid_values.clone(),
            ))
        } else {
            None
        };

        let chart_width = 800.0;
        let chart_height = 500.0;

        let freq_scale = LinearScale::new()
            .domain(log_freq_min, log_freq_max)
            .range(0.0, chart_width as f64);

        let angle_scale = LinearScale::new()
            .domain(angle_min, angle_max)
            .range(0.0, chart_height as f64);

        // Configure rendering based on mode
        let color_scale = colormap.color_scale();
        let contour_config = ContourConfig::new()
            .stroke_width(if is_isoline { 1.5 } else { 0.5 })
            .fill(is_surface)
            .fill_opacity(if is_surface {
                0.6
            } else if is_heatmap {
                1.0
            } else {
                0.0
            })
            .stroke_opacity(if is_isoline { 1.0 } else { 0.8 })
            .color_scale(color_scale);

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
                                        // In Isoline mode, render grid first (underneath lines)
                                        .when(is_isoline, |el| {
                                            el.child(render_grid(
                                                &freq_scale,
                                                &angle_scale,
                                                &GridConfig::with_lines()
                                                    .with_vertical_values(freq_ticks.clone()),
                                                chart_width,
                                                chart_height,
                                                &theme,
                                            ))
                                        })
                                        // Render contour bands (for Surface mode) - filled polygons
                                        .when_some(contour_bands.clone(), |el, bands| {
                                            el.child(
                                                render_contour_bands(
                                                    bands,
                                                    &freq_scale,
                                                    &angle_scale,
                                                    &contour_config,
                                                )
                                                .value_range(spl_min, spl_max)
                                                .height(px(chart_height as f32)),
                                            )
                                        })
                                        // Render heatmap (for Heatmap mode) - uses quads, no anti-aliasing gaps
                                        .when_some(heatmap_data.clone(), |el, data| {
                                            el.child(
                                                render_heatmap(
                                                    data,
                                                    &freq_scale,
                                                    &angle_scale,
                                                    &contour_config,
                                                )
                                                .value_range(spl_min, spl_max)
                                                .height(px(chart_height as f32)),
                                            )
                                        })
                                        // In Surface/Heatmap mode, render grid on top
                                        .when(is_heatmap, |el| {
                                            el.child(render_grid(
                                                &freq_scale,
                                                &angle_scale,
                                                &GridConfig::with_lines()
                                                    .with_vertical_values(freq_ticks.clone()),
                                                chart_width,
                                                chart_height,
                                                &theme,
                                            ))
                                        })
                                        // Render contour lines (for Isoline mode)
                                        .when_some(contours.clone(), |el, c| {
                                            el.child(
                                                render_contour(
                                                    c,
                                                    &freq_scale,
                                                    &angle_scale,
                                                    &contour_config,
                                                )
                                                .value_range(spl_min, spl_max)
                                                .height(px(chart_height as f32)),
                                            )
                                        }),
                                ),
                        )
                        .child(
                            div().flex().child(div().w(px(80.0))).child(render_axis(
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
                        .child(render_vector_text(
                            &format!("{:.0} dB", spl_min),
                            &font_config,
                        ))
                        .children((0..15).map(|i| {
                            let t = i as f64 / 14.0;
                            let color = colormap.color_scale()(t);
                            let (r, g, b) = (
                                (color.r * 255.0) as u32,
                                (color.g * 255.0) as u32,
                                (color.b * 255.0) as u32,
                            );
                            div()
                                .w(px(15.0))
                                .h(px(15.0))
                                .bg(rgb((r << 16) | (g << 8) | b))
                        }))
                        .child(render_vector_text(
                            &format!("{:.0} dB", spl_max),
                            &font_config,
                        ))
                }),
        )
    }

    fn render_contour_plot(&mut self, cx: &mut Context<Self>) -> Div {
        let has_contour_data = self.contour_data.is_some();
        let has_directivity_data = self
            .directivity_data
            .as_ref()
            .map_or(false, |d| !d.horizontal.is_empty());

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
                app.contour_mode_spl = app.contour_mode_spl.next();
            },
            cx,
        );

        let directivity_toggle = self.render_mode_toggle(
            directivity_mode,
            "directivity-contour-toggle",
            |app, _cx| {
                app.contour_mode_directivity = app.contour_mode_directivity.next();
            },
            cx,
        );

        // Pre-render the contour plots
        let colormap = self.contour_colormap;
        let spl_contour = self.render_contour_from_contour_data(
            "SPL Horizontal Contour (Full 360°)",
            spl_mode,
            colormap,
        );
        let directivity_contour = self.render_contour_from_directivity(
            "Directivity Contour (SPL Horizontal)",
            directivity_mode,
            colormap,
        );

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
                        .child(contour_div),
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
                        .child(contour_div),
                )
            })
    }

    /// Render polar directivity plot - SPL vs angle at selected frequencies
    fn render_polar_directivity_plot(&mut self, cx: &mut Context<Self>) -> Div {
        let Some(ref directivity) = self.directivity_data else {
            return div().flex().items_center().justify_center().h_full().child(
                div()
                    .text_base()
                    .text_color(rgb(0x666666))
                    .child("No directivity data available for this speaker."),
            );
        };

        let chart_size = 500.0_f32;
        let center_x = chart_size / 2.0;
        let center_y = chart_size / 2.0;
        let outer_radius = (chart_size / 2.0) - 60.0; // Leave margin for labels

        // SPL range for normalization (0 dB at outer edge, -30 dB at center)
        let spl_max = 0.0_f64;
        let spl_min = -30.0_f64;
        let spl_range = spl_max - spl_min;

        // Get angle range from data
        let use_horizontal = self.polar_plane == DirectivityPlane::Horizontal;
        let (_angle_min, _angle_max) = get_angle_range(directivity, use_horizontal);

        // Colors for different frequencies (use viridis-like palette)
        let freq_colors = [
            D3Color::from_hex(0x440154), // Dark purple
            D3Color::from_hex(0x3b528b), // Blue-purple
            D3Color::from_hex(0x21918c), // Teal
            D3Color::from_hex(0x5ec962), // Green
            D3Color::from_hex(0xfde725), // Yellow
        ];

        // Build paths for each selected frequency
        let mut frequency_paths: Vec<(f64, Vec<(f32, f32)>, D3Color)> = Vec::new();

        for (i, &freq) in self.polar_selected_frequencies.iter().take(5).enumerate() {
            let color = freq_colors[i % freq_colors.len()];

            // Get SPL at each angle for this frequency
            let angle_spl_pairs = interpolate_spl_at_frequency(directivity, freq, use_horizontal);

            if angle_spl_pairs.is_empty() {
                continue;
            }

            // Convert to screen coordinates
            let points: Vec<(f32, f32)> = angle_spl_pairs
                .iter()
                .map(|&(angle_deg, spl)| {
                    // Convert angle: 0° at top means -PI/2 offset
                    let angle_rad = (angle_deg - 90.0).to_radians();
                    // Normalize SPL to radius (spl_max -> outer_radius, spl_min -> 0)
                    let normalized_spl = ((spl - spl_min) / spl_range).clamp(0.0, 1.0);
                    let radius = normalized_spl as f32 * outer_radius;
                    let x = center_x + radius * angle_rad.cos() as f32;
                    let y = center_y + radius * angle_rad.sin() as f32;
                    (x, y)
                })
                .collect();

            frequency_paths.push((freq, points, color));
        }

        // Generate polar grid paths as screen coordinates
        let grid_radii: Vec<f32> = vec![0.25, 0.5, 0.75, 1.0]
            .iter()
            .map(|&t| t * outer_radius)
            .collect();

        let grid_angles: Vec<f64> = (0..12).map(|i| (i as f64 * 30.0).to_radians()).collect();

        // Clone data for the canvas closure
        let frequency_paths_clone = frequency_paths.clone();
        let grid_radii_clone = grid_radii.clone();
        let grid_angles_clone = grid_angles.clone();

        // Canvas-based polar plot
        let polar_canvas = canvas(
            move |_bounds, _window, _cx| {
                // Prepaint: just pass through the data
                (
                    frequency_paths_clone.clone(),
                    grid_radii_clone.clone(),
                    grid_angles_clone.clone(),
                    center_x,
                    center_y,
                    outer_radius,
                )
            },
            move |bounds, (freq_paths, radii, angles, cx_f, cy_f, outer_r), window, _cx| {
                let origin_x: f32 = bounds.origin.x.into();
                let origin_y: f32 = bounds.origin.y.into();

                // Draw grid circles
                for &r in &radii {
                    let num_segments = 72;
                    let mut builder = PathBuilder::stroke(px(1.0));
                    for i in 0..=num_segments {
                        let theta = (i as f32 / num_segments as f32) * std::f32::consts::TAU;
                        let x = origin_x + cx_f + r * theta.cos();
                        let y = origin_y + cy_f + r * theta.sin();
                        if i == 0 {
                            builder.move_to(point(px(x), px(y)));
                        } else {
                            builder.line_to(point(px(x), px(y)));
                        }
                    }
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, hsla(0.0, 0.0, 0.8, 1.0));
                    }
                }

                // Draw grid rays
                for &angle in &angles {
                    let mut builder = PathBuilder::stroke(px(1.0));
                    let x1 = origin_x + cx_f;
                    let y1 = origin_y + cy_f;
                    let x2 = origin_x + cx_f + outer_r * angle.cos() as f32;
                    let y2 = origin_y + cy_f + angle.sin() as f32 * outer_r;
                    builder.move_to(point(px(x1), px(y1)));
                    builder.line_to(point(px(x2), px(y2)));
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, hsla(0.0, 0.0, 0.85, 1.0));
                    }
                }

                // Draw frequency curves
                for (_, points, color) in &freq_paths {
                    if points.len() < 2 {
                        continue;
                    }
                    let mut builder = PathBuilder::stroke(px(2.0));
                    for (i, &(x, y)) in points.iter().enumerate() {
                        let screen_x = origin_x + x;
                        let screen_y = origin_y + y;
                        if i == 0 {
                            builder.move_to(point(px(screen_x), px(screen_y)));
                        } else {
                            builder.line_to(point(px(screen_x), px(screen_y)));
                        }
                    }
                    // Close the path
                    if let Some(&(x, y)) = points.first() {
                        builder.line_to(point(px(origin_x + x), px(origin_y + y)));
                    }
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, color.to_rgba());
                    }
                }
            },
        )
        .w(px(chart_size))
        .h(px(chart_size));

        // Build legend
        let legend =
            div()
                .flex()
                .flex_row()
                .gap_4()
                .justify_center()
                .children(frequency_paths.iter().map(|(freq, _, color)| {
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w(px(16.0))
                                .h(px(3.0))
                                .bg(color.to_rgba())
                                .rounded(px(1.0)),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x666666))
                                .child(format!("{}", format_frequency(*freq))),
                        )
                }));

        // Angle labels using render_vector_text
        let font_config = VectorFontConfig::horizontal(10.0, hsla(0.0, 0.0, 0.4, 1.0));
        let angle_labels = div().absolute().inset_0().children((0..12).map(|i| {
            let angle_deg = i as f64 * 30.0;
            let angle_rad = (angle_deg - 90.0).to_radians();
            let label_radius = outer_radius + 25.0;
            let x = center_x + label_radius * angle_rad.cos() as f32;
            let y = center_y + label_radius * angle_rad.sin() as f32;

            let display_angle = if angle_deg <= 180.0 {
                angle_deg as i32
            } else {
                (angle_deg - 360.0) as i32
            };

            div()
                .absolute()
                .left(px(x - 15.0))
                .top(px(y - 6.0))
                .child(render_vector_text(
                    &format!("{}°", display_angle),
                    &font_config,
                ))
        }));

        // dB labels on radial axis
        let db_font_config = VectorFontConfig::horizontal(9.0, hsla(0.0, 0.0, 0.6, 1.0));
        let db_labels = div()
            .absolute()
            .inset_0()
            .children([0.0, -10.0, -20.0, -30.0].iter().map(|&db| {
                let normalized = (db - spl_min) / spl_range;
                let r = normalized as f32 * outer_radius;
                let x = center_x;
                let y = center_y - r - 8.0;

                div()
                    .absolute()
                    .left(px(x - 20.0))
                    .top(px(y))
                    .child(render_vector_text(
                        &format!("{} dB", db as i32),
                        &db_font_config,
                    ))
            }));

        // Frequency selection controls
        let available_frequencies: Vec<f64> = vec![
            100.0, 200.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 10000.0, 16000.0,
        ];

        let freq_selector = div().flex().flex_row().flex_wrap().gap_2().children(
            available_frequencies.iter().enumerate().map(|(i, &freq)| {
                let is_selected = self.polar_selected_frequencies.contains(&freq);
                let freq_clone = freq;
                div()
                    .id(ElementId::NamedInteger("polar-freq".into(), i as u64))
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .border_1()
                    .when(is_selected, |el| {
                        el.bg(rgb(0x3b82f6))
                            .text_color(rgb(0xffffff))
                            .border_color(rgb(0x3b82f6))
                    })
                    .when(!is_selected, |el| {
                        el.bg(rgb(0xffffff))
                            .text_color(rgb(0x666666))
                            .border_color(rgb(0xdddddd))
                    })
                    .child(format_frequency(freq))
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        if this.polar_selected_frequencies.contains(&freq_clone) {
                            this.polar_selected_frequencies.retain(|&f| f != freq_clone);
                        } else if this.polar_selected_frequencies.len() < 5 {
                            this.polar_selected_frequencies.push(freq_clone);
                            this.polar_selected_frequencies
                                .sort_by(|a, b| a.partial_cmp(b).unwrap());
                        }
                        cx.notify();
                    }))
            }),
        );

        // Plane toggle
        let plane_toggle = div()
            .flex()
            .flex_row()
            .gap_2()
            .child(
                div()
                    .id("polar-plane-horizontal")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .when(self.polar_plane == DirectivityPlane::Horizontal, |el| {
                        el.bg(rgb(0x3b82f6)).text_color(rgb(0xffffff))
                    })
                    .when(self.polar_plane != DirectivityPlane::Horizontal, |el| {
                        el.bg(rgb(0xe5e7eb)).text_color(rgb(0x666666))
                    })
                    .child("Horizontal")
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.polar_plane = DirectivityPlane::Horizontal;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("polar-plane-vertical")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .when(self.polar_plane == DirectivityPlane::Vertical, |el| {
                        el.bg(rgb(0x3b82f6)).text_color(rgb(0xffffff))
                    })
                    .when(self.polar_plane != DirectivityPlane::Vertical, |el| {
                        el.bg(rgb(0xe5e7eb)).text_color(rgb(0x666666))
                    })
                    .child("Vertical")
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.polar_plane = DirectivityPlane::Vertical;
                        cx.notify();
                    })),
            );

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
                        "Polar Directivity - {}",
                        self.selected_speaker.as_deref().unwrap_or("Unknown")
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .items_center()
                    .child(div().text_sm().text_color(rgb(0x666666)).child("Plane:"))
                    .child(plane_toggle)
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x666666))
                            .ml_4()
                            .child("Frequencies:"),
                    )
                    .child(freq_selector),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .relative()
                            .w(px(chart_size))
                            .h(px(chart_size))
                            .child(polar_canvas)
                            .child(angle_labels)
                            .child(db_labels),
                    )
                    .child(legend),
            )
    }

    /// Render 3D surface plot - SPL as a function of frequency and angle
    fn render_surface_3d_plot(&mut self, cx: &mut Context<Self>) -> Div {
        let Some(ref contour_data) = self.contour_data else {
            return div().flex().items_center().justify_center().h_full().child(
                div()
                    .text_base()
                    .text_color(rgb(0x666666))
                    .child("No contour data available for this speaker."),
            );
        };

        let chart_width = 800.0;
        let chart_height = 500.0;

        // Build surface data from contour data
        let freq_min = contour_data.freq.first().copied().unwrap_or(20.0);
        let freq_max = contour_data.freq.last().copied().unwrap_or(20000.0);
        let angle_min = contour_data.angles.first().copied().unwrap_or(-180.0);
        let angle_max = contour_data.angles.last().copied().unwrap_or(180.0);

        // Create a closure that interpolates from the contour data
        let freq_count = contour_data.freq_count;
        let angle_count = contour_data.angle_count;
        let freq_values = contour_data.freq.clone();
        let angle_values = contour_data.angles.clone();
        let spl_values = contour_data.spl.clone();

        // Calculate SPL range for axis labels
        let spl_min = spl_values.iter().cloned().fold(f64::INFINITY, f64::min);
        let spl_max = spl_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        let surface_data = SurfaceData::from_z_function_logx(
            (freq_min.max(20.0), freq_max.min(20000.0)),
            (angle_min, angle_max),
            80, // Resolution
            |freq, angle| {
                // Find nearest indices in the contour data grid
                let freq_idx = freq_values
                    .iter()
                    .position(|&f| f >= freq)
                    .unwrap_or(freq_count - 1)
                    .min(freq_count - 1);
                let angle_idx = angle_values
                    .iter()
                    .position(|&a| a >= angle)
                    .unwrap_or(angle_count - 1)
                    .min(angle_count - 1);

                // Get SPL value (row-major: angle_idx * freq_count + freq_idx)
                let idx = angle_idx * freq_count + freq_idx;
                spl_values.get(idx).copied().unwrap_or(0.0)
            },
        );

        // Map our SurfaceProjection to d3rs projection type
        let config = match self.surface_projection {
            SurfaceProjection::Isometric => SurfaceConfig::new().isometric(),
            SurfaceProjection::Perspective => SurfaceConfig::new().perspective(),
            SurfaceProjection::Orthographic => SurfaceConfig::new().orthographic(),
            SurfaceProjection::Oblique => SurfaceConfig::new().oblique(),
        };

        let config = config
            .rotation(
                self.surface_rotation_elevation as f64,
                self.surface_rotation_azimuth as f64,
            )
            .wireframe(self.surface_wireframe)
            .color_scale(ColorScaleType::Viridis)
            .opacity(0.6)
            .scale(1.2)
            .show_axes(true)
            .axis_color(D3Color::rgb(80, 80, 80))
            .axis_width(2.0)
            .axis_labels("Freq", "Angle", "SPL")
            .axis_ranges(
                (freq_min.max(20.0), freq_max.min(20000.0)),
                (angle_min, angle_max),
                (spl_min, spl_max),
            )
            .axis_font_size(9.0);

        let surface_element = render_surface(&surface_data, config, chart_width, chart_height);

        // Projection selector
        let projections = [
            (SurfaceProjection::Isometric, "Isometric"),
            (SurfaceProjection::Perspective, "Perspective"),
            (SurfaceProjection::Orthographic, "Orthographic"),
            (SurfaceProjection::Oblique, "Oblique"),
        ];

        let projection_selector =
            div()
                .flex()
                .flex_row()
                .gap_2()
                .children(projections.iter().enumerate().map(|(i, &(proj, label))| {
                    div()
                        .id(ElementId::NamedInteger(
                            "surface-projection".into(),
                            i as u64,
                        ))
                        .px_3()
                        .py_1()
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .when(self.surface_projection == proj, |el| {
                            el.bg(rgb(0x3b82f6)).text_color(rgb(0xffffff))
                        })
                        .when(self.surface_projection != proj, |el| {
                            el.bg(rgb(0xe5e7eb)).text_color(rgb(0x666666))
                        })
                        .child(label)
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.surface_projection = proj;
                            cx.notify();
                        }))
                }));

        // Wireframe toggle
        let wireframe_toggle = div()
            .id("surface-wireframe-toggle")
            .px_3()
            .py_1()
            .rounded(px(4.0))
            .cursor_pointer()
            .when(self.surface_wireframe, |el| {
                el.bg(rgb(0x3b82f6)).text_color(rgb(0xffffff))
            })
            .when(!self.surface_wireframe, |el| {
                el.bg(rgb(0xe5e7eb)).text_color(rgb(0x666666))
            })
            .child("Wireframe")
            .on_click(cx.listener(|this, _, _window, cx| {
                this.surface_wireframe = !this.surface_wireframe;
                cx.notify();
            }));

        // Rotation controls
        let azimuth_slider = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(div().text_sm().text_color(rgb(0x666666)).child("Azimuth:"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x333333))
                    .child(format!("{}°", self.surface_rotation_azimuth as i32)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .child(
                        div()
                            .id("azimuth-dec")
                            .px_2()
                            .py_1()
                            .bg(rgb(0xe5e7eb))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .child("-")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.surface_rotation_azimuth =
                                    (this.surface_rotation_azimuth - 15.0).rem_euclid(360.0);
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("azimuth-inc")
                            .px_2()
                            .py_1()
                            .bg(rgb(0xe5e7eb))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .child("+")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.surface_rotation_azimuth =
                                    (this.surface_rotation_azimuth + 15.0).rem_euclid(360.0);
                                cx.notify();
                            })),
                    ),
            );

        let elevation_slider = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x666666))
                    .child("Elevation:"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x333333))
                    .child(format!("{}°", self.surface_rotation_elevation as i32)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .child(
                        div()
                            .id("elevation-dec")
                            .px_2()
                            .py_1()
                            .bg(rgb(0xe5e7eb))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .child("-")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.surface_rotation_elevation =
                                    (this.surface_rotation_elevation - 15.0).clamp(-90.0, 90.0);
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("elevation-inc")
                            .px_2()
                            .py_1()
                            .bg(rgb(0xe5e7eb))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .child("+")
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.surface_rotation_elevation =
                                    (this.surface_rotation_elevation + 15.0).clamp(-90.0, 90.0);
                                cx.notify();
                            })),
                    ),
            );

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
                        "3D Surface - {}",
                        self.selected_speaker.as_deref().unwrap_or("Unknown")
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_4()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x666666))
                            .child("Projection:"),
                    )
                    .child(projection_selector)
                    .child(wireframe_toggle)
                    .child(azimuth_slider)
                    .child(elevation_slider),
            )
            .child(div().flex().justify_center().child(surface_element))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_center()
                    .gap_4()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x666666))
                            .child("X: Frequency (Hz, log scale)"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x666666))
                            .child("Y: Angle (degrees)"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x666666))
                            .child("Z/Color: SPL (dB)"),
                    ),
            )
    }

    /// Render polar contour plot - contour in polar coordinates (r=frequency, θ=angle)
    fn render_polar_contour_plot(&mut self, cx: &mut Context<Self>) -> Div {
        let Some(ref contour_data) = self.contour_data else {
            return div().flex().items_center().justify_center().h_full().child(
                div()
                    .text_base()
                    .text_color(rgb(0x666666))
                    .child("No contour data available for this speaker."),
            );
        };

        let chart_size = 600.0_f32;
        let center_x = chart_size / 2.0;
        let center_y = chart_size / 2.0;
        let outer_radius = (chart_size / 2.0) - 80.0;

        // Frequency range (logarithmic radial axis)
        let freq_min = self.polar_contour_freq_range.0.max(20.0);
        let freq_max = self.polar_contour_freq_range.1.min(20000.0);
        let log_freq_min = freq_min.ln();
        let log_freq_max = freq_max.ln();

        // Color scale
        let color_scale = self.contour_colormap.color_scale();

        // Find SPL range for color normalization
        let spl_min = contour_data
            .spl
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let spl_max = contour_data
            .spl
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        let spl_range = (spl_max - spl_min).max(1.0);

        // Generate polar heatmap data - store wedge geometry and colors
        let angular_resolution = 72_usize; // 5° steps
        let radial_resolution = 50_usize;

        // Build wedge data for canvas rendering
        #[derive(Clone)]
        struct WedgeData {
            r_inner: f32,
            r_outer: f32,
            theta1: f32,
            theta2: f32,
            color: Rgba,
        }

        let mut wedges: Vec<WedgeData> = Vec::with_capacity(angular_resolution * radial_resolution);

        let angle_min_data = contour_data.angles.first().copied().unwrap_or(-180.0);
        let angle_max_data = contour_data.angles.last().copied().unwrap_or(180.0);

        for r_idx in 0..radial_resolution {
            for a_idx in 0..angular_resolution {
                // Calculate frequency for this radial position (logarithmic)
                let r_t = r_idx as f64 / radial_resolution as f64;
                let r_t_next = (r_idx + 1) as f64 / radial_resolution as f64;

                let log_freq = log_freq_min + r_t * (log_freq_max - log_freq_min);
                let freq = log_freq.exp();

                // Calculate angle (0-360)
                let angle_t = a_idx as f64 / angular_resolution as f64;
                let angle_t_next = (a_idx + 1) as f64 / angular_resolution as f64;

                // Map to data angle range
                let angle = angle_min_data + angle_t * (angle_max_data - angle_min_data);

                // Find nearest indices in contour data
                let freq_idx = contour_data
                    .freq
                    .iter()
                    .position(|&f| f >= freq)
                    .unwrap_or(contour_data.freq_count - 1)
                    .min(contour_data.freq_count - 1);

                let angle_idx = contour_data
                    .angles
                    .iter()
                    .position(|&a| a >= angle)
                    .unwrap_or(contour_data.angle_count - 1)
                    .min(contour_data.angle_count - 1);

                let idx = angle_idx * contour_data.freq_count + freq_idx;
                let spl = contour_data.spl.get(idx).copied().unwrap_or(0.0);

                // Normalize SPL to color
                let color_t = ((spl - spl_min) / spl_range).clamp(0.0, 1.0);
                let color = color_scale(color_t);

                // Calculate radii (inner and outer)
                let r_inner = (r_t * outer_radius as f64) as f32;
                let r_outer = (r_t_next * outer_radius as f64) as f32;

                // Calculate angles (convert to radians, 0° at top)
                let theta1 = ((angle_t * 360.0 - 90.0).to_radians()) as f32;
                let theta2 = ((angle_t_next * 360.0 - 90.0).to_radians()) as f32;

                wedges.push(WedgeData {
                    r_inner,
                    r_outer,
                    theta1,
                    theta2,
                    color: color.to_rgba(),
                });
            }
        }

        // Grid frequencies for overlay
        let grid_frequencies = [100.0_f64, 1000.0, 10000.0];
        let grid_radii: Vec<f32> = grid_frequencies
            .iter()
            .filter(|&&f| f >= freq_min && f <= freq_max)
            .map(|&f| {
                let t = (f.ln() - log_freq_min) / (log_freq_max - log_freq_min);
                (t * outer_radius as f64) as f32
            })
            .collect();

        let grid_angles: Vec<f32> = (0..12)
            .map(|i| ((i as f64 * 30.0).to_radians()) as f32)
            .collect();

        // Clone data for canvas closure
        let wedges_clone = wedges.clone();
        let grid_radii_clone = grid_radii.clone();
        let grid_angles_clone = grid_angles.clone();

        // Canvas-based polar contour plot
        let polar_canvas = canvas(
            move |_bounds, _window, _cx| {
                (
                    wedges_clone.clone(),
                    grid_radii_clone.clone(),
                    grid_angles_clone.clone(),
                    center_x,
                    center_y,
                    outer_radius,
                )
            },
            move |bounds, (wedge_data, radii, angles, cx_f, cy_f, outer_r), window, _cx| {
                let origin_x: f32 = bounds.origin.x.into();
                let origin_y: f32 = bounds.origin.y.into();

                // Draw wedges (filled quads approximating arcs)
                for wedge in &wedge_data {
                    // For small angular segments, approximate with a quad
                    let x1_inner = origin_x + cx_f + wedge.r_inner * wedge.theta1.cos();
                    let y1_inner = origin_y + cy_f + wedge.r_inner * wedge.theta1.sin();
                    let x2_inner = origin_x + cx_f + wedge.r_inner * wedge.theta2.cos();
                    let y2_inner = origin_y + cy_f + wedge.r_inner * wedge.theta2.sin();
                    let x1_outer = origin_x + cx_f + wedge.r_outer * wedge.theta1.cos();
                    let y1_outer = origin_y + cy_f + wedge.r_outer * wedge.theta1.sin();
                    let x2_outer = origin_x + cx_f + wedge.r_outer * wedge.theta2.cos();
                    let y2_outer = origin_y + cy_f + wedge.r_outer * wedge.theta2.sin();

                    // Build a filled quad path
                    let mut builder = PathBuilder::fill();
                    builder.move_to(point(px(x1_inner), px(y1_inner)));
                    builder.line_to(point(px(x1_outer), px(y1_outer)));
                    builder.line_to(point(px(x2_outer), px(y2_outer)));
                    builder.line_to(point(px(x2_inner), px(y2_inner)));
                    builder.line_to(point(px(x1_inner), px(y1_inner))); // Close

                    if let Ok(path) = builder.build() {
                        window.paint_path(path, wedge.color);
                    }
                }

                // Draw grid circles
                for &r in &radii {
                    let num_segments = 72;
                    let mut builder = PathBuilder::stroke(px(1.0));
                    for i in 0..=num_segments {
                        let theta = (i as f32 / num_segments as f32) * std::f32::consts::TAU;
                        let x = origin_x + cx_f + r * theta.cos();
                        let y = origin_y + cy_f + r * theta.sin();
                        if i == 0 {
                            builder.move_to(point(px(x), px(y)));
                        } else {
                            builder.line_to(point(px(x), px(y)));
                        }
                    }
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, hsla(0.0, 0.0, 1.0, 0.4));
                    }
                }

                // Draw grid rays
                for &angle in &angles {
                    let mut builder = PathBuilder::stroke(px(1.0));
                    let x1 = origin_x + cx_f;
                    let y1 = origin_y + cy_f;
                    let x2 = origin_x + cx_f + outer_r * angle.cos();
                    let y2 = origin_y + cy_f + outer_r * angle.sin();
                    builder.move_to(point(px(x1), px(y1)));
                    builder.line_to(point(px(x2), px(y2)));
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, hsla(0.0, 0.0, 1.0, 0.25));
                    }
                }
            },
        )
        .w(px(chart_size))
        .h(px(chart_size));

        // Frequency labels using render_vector_text
        let font_config = VectorFontConfig::horizontal(10.0, hsla(0.0, 0.0, 0.2, 1.0));
        let freq_labels = div().absolute().inset_0().children(
            grid_frequencies
                .iter()
                .filter(|&&f| f >= freq_min && f <= freq_max)
                .map(|&freq| {
                    let t = (freq.ln() - log_freq_min) / (log_freq_max - log_freq_min);
                    let r = (t * outer_radius as f64) as f32;
                    let x = center_x;
                    let y = center_y - r - 8.0;

                    div()
                        .absolute()
                        .left(px(x - 15.0))
                        .top(px(y))
                        .child(render_vector_text(&format_frequency(freq), &font_config))
                }),
        );

        // Angle labels
        let angle_font_config = VectorFontConfig::horizontal(10.0, hsla(0.0, 0.0, 0.4, 1.0));
        let angle_labels = div().absolute().inset_0().children((0..12).map(|i| {
            let angle_deg = i as f64 * 30.0;
            let angle_rad = (angle_deg - 90.0).to_radians();
            let label_radius = outer_radius + 25.0;
            let x = center_x + label_radius * angle_rad.cos() as f32;
            let y = center_y + label_radius * angle_rad.sin() as f32;

            // Map display angle to data angle convention
            let display_angle =
                angle_min_data + (angle_deg / 360.0) * (angle_max_data - angle_min_data);

            div()
                .absolute()
                .left(px(x - 15.0))
                .top(px(y - 6.0))
                .child(render_vector_text(
                    &format!("{:.0}°", display_angle),
                    &angle_font_config,
                ))
        }));

        // Colorbar using div elements
        let colorbar_height = 200.0_f32;
        let colorbar_width = 20.0_f32;
        let num_color_steps = 20;

        let colorbar = div()
            .flex()
            .flex_col()
            .gap_1()
            .children((0..num_color_steps).map(|i| {
                let t = i as f64 / num_color_steps as f64;
                let color = color_scale(1.0 - t); // Invert so high values at top
                let h = colorbar_height / num_color_steps as f32;

                div().w(px(colorbar_width)).h(px(h)).bg(color.to_rgba())
            }));

        // Colorbar labels
        let colorbar_label_font = VectorFontConfig::horizontal(10.0, hsla(0.0, 0.0, 0.4, 1.0));
        let colorbar_labels = div()
            .flex()
            .flex_col()
            .justify_between()
            .h(px(colorbar_height))
            .children([0.0, 0.5, 1.0].iter().map(|&t| {
                let spl_val = spl_min + t * spl_range;
                div().child(render_vector_text(
                    &format!("{:.0} dB", spl_val),
                    &colorbar_label_font,
                ))
            }));

        // Colormap selector
        let colormaps = [
            (Colormap::Viridis, "Viridis"),
            (Colormap::Plasma, "Plasma"),
            (Colormap::Magma, "Magma"),
            (Colormap::Inferno, "Inferno"),
            (Colormap::Heat, "Heat"),
            (Colormap::Coolwarm, "Coolwarm"),
        ];

        let colormap_selector =
            div()
                .flex()
                .flex_row()
                .gap_2()
                .children(colormaps.iter().enumerate().map(|(i, &(cmap, label))| {
                    div()
                        .id(ElementId::NamedInteger(
                            "polar-contour-colormap".into(),
                            i as u64,
                        ))
                        .px_3()
                        .py_1()
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .when(self.contour_colormap == cmap, |el| {
                            el.bg(rgb(0x3b82f6)).text_color(rgb(0xffffff))
                        })
                        .when(self.contour_colormap != cmap, |el| {
                            el.bg(rgb(0xe5e7eb)).text_color(rgb(0x666666))
                        })
                        .child(label)
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.contour_colormap = cmap;
                            cx.notify();
                        }))
                }));

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
                        "Polar Contour - {}",
                        self.selected_speaker.as_deref().unwrap_or("Unknown")
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .items_center()
                    .child(div().text_sm().text_color(rgb(0x666666)).child("Colormap:"))
                    .child(colormap_selector),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_center()
                    .gap_8()
                    .child(
                        div()
                            .relative()
                            .w(px(chart_size))
                            .h(px(chart_size))
                            .child(polar_canvas)
                            .child(freq_labels)
                            .child(angle_labels),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .child(div().text_sm().text_color(rgb(0x666666)).child("SPL (dB)"))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(colorbar)
                                    .child(colorbar_labels),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_center()
                    .gap_4()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x666666))
                            .child("Radial: Frequency (log scale)"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x666666))
                            .child("Angular: Angle (degrees)"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x666666))
                            .child("Color: SPL (dB)"),
                    ),
            )
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
