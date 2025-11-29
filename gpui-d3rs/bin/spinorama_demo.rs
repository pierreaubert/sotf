//! Spinorama Demo - Speaker frequency response visualization
//!
//! Demonstrates fetching and plotting speaker measurement data from spinorama.org.

use std::collections::HashMap;
use std::sync::Arc;

use autoeq::read::{
    extract_cea2034_curves_original, fetch_available_speakers, fetch_directivity_data,
    fetch_measurement_plot_data,
};
use autoeq::{Curve, DirectivityData};
use d3rs::axis::{render_axis, AxisConfig, DefaultAxisTheme};
use d3rs::color::D3Color;
use d3rs::grid::{render_grid, GridConfig};
use d3rs::prelude::*;
use d3rs::shape::LineConfig;
use gpui::prelude::*;
use gpui::{deferred, *};
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
    data_load_state: LoadState,
    // UI state
    current_section: PlotSection,
    speaker_dropdown_open: bool,
    version_dropdown_open: bool,
    section_dropdown_open: bool,
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
            data_load_state: LoadState::Idle,
            current_section: PlotSection::default(),
            speaker_dropdown_open: false,
            version_dropdown_open: false,
            section_dropdown_open: false,
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
                .spawn(async {
                    fetch_available_speakers()
                        .await
                        .map_err(|e| e.to_string())
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r);

            match result {
                Ok(speakers) => {
                    println!("Loaded {} speakers", speakers.len());
                    let _ = this.update(cx, |app, cx| {
                        app.speakers = speakers;
                        app.speakers_load_state = LoadState::Loaded;
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
                        let versions: Vec<String> = response.json().await.map_err(|e| e.to_string())?;
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
                        let plot_data = fetch_measurement_plot_data(&speaker, &version, &measurement)
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

            match cea2034_result {
                Ok(curves) => {
                    let _ = this.update(cx, |app, cx| {
                        app.cea2034_curves = curves;
                        app.directivity_data = directivity_result;
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
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0xcccccc))
                            .child("Speaker:"),
                    )
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
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0xcccccc))
                                .child("Version:"),
                        )
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
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0xcccccc))
                            .child("Plot:"),
                    )
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
                            .child(current.clone().unwrap_or_else(|| "Select speaker...".into())),
                    )
                    .child(div().text_xs().text_color(rgb(0x666666)).child("▼"))
                    .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                        println!("Speaker dropdown clicked!");
                        entity_for_toggle.update(cx, |this, cx| {
                            this.speaker_dropdown_open = !this.speaker_dropdown_open;
                            this.version_dropdown_open = false;
                            this.section_dropdown_open = false;
                            println!("Speaker dropdown open: {}, speakers count: {}", this.speaker_dropdown_open, this.speakers.len());
                            cx.notify();
                        });
                    }),
            )
            .when(is_open, |el| {
                el.child(deferred(
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
                                    el.text_color(rgb(0xcccccc))
                                        .hover(|s| s.bg(rgb(0x3a3a3a)))
                                })
                                .child(opt.label)
                                .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.selected_speaker = Some(value.clone());
                                        this.speaker_dropdown_open = false;
                                        // Clear previous data when changing speaker
                                        this.cea2034_curves.clear();
                                        this.directivity_data = None;
                                        this.data_load_state = LoadState::Idle;
                                        // Load versions for this speaker
                                        this.load_versions(cx);
                                    });
                                })
                        })),
                ).with_priority(1))
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
                            .child(current.clone().unwrap_or_else(|| "Select version...".into())),
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
                el.child(deferred(
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
                                    el.text_color(rgb(0xcccccc))
                                        .hover(|s| s.bg(rgb(0x3a3a3a)))
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
                ).with_priority(1))
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
                el.child(deferred(
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
                                    el.text_color(rgb(0xcccccc))
                                        .hover(|s| s.bg(rgb(0x3a3a3a)))
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
                ).with_priority(1))
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
                PlotSection::Contour => self.render_contour_plot(),
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
        let theme = DefaultAxisTheme;
        let colors = cea2034_colors();

        // Create log frequency scale (20Hz - 20kHz)
        let freq_scale = LogScale::new().domain(20.0, 20000.0).range(0.0, 800.0);
        // Create linear SPL scale
        let spl_scale = LinearScale::new().domain(-40.0, 10.0).range(0.0, 400.0);

        let chart_width = 800.0;
        let chart_height = 400.0;

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
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .child(render_axis(
                                &spl_scale,
                                &AxisConfig::left()
                                    .with_ticks(10)
                                    .with_formatter(|v| format!("{:.0} dB", v)),
                                chart_height,
                                &theme,
                            ))
                            .child(
                                div()
                                    .w(px(chart_width as f32))
                                    .h(px(chart_height as f32))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .border_1()
                                    .border_color(rgb(0xcccccc))
                                    .child(render_grid(
                                        &freq_scale,
                                        &spl_scale,
                                        &GridConfig::with_lines(),
                                        chart_width,
                                        chart_height,
                                        &theme,
                                    ))
                                    .children(CEA2034_CURVES.iter().filter_map(|&name| {
                                        let curve = self.cea2034_curves.get(name)?;
                                        let color = colors.get(name).cloned().unwrap_or(D3Color::rgb(128, 128, 128));

                                        // Convert curve to line points
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

                                        Some(render_line(
                                            &freq_scale,
                                            &spl_scale,
                                            &points,
                                            &LineConfig::new()
                                                .stroke_color(color)
                                                .stroke_width(2.0)
                                                .curve(CurveType::Linear),
                                        ))
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .ml(px(60.0))
                            .child(render_axis(
                                &freq_scale,
                                &AxisConfig::bottom()
                                    .with_ticks(10)
                                    .with_formatter(|f| {
                                        if f >= 1000.0 {
                                            format!("{:.0}k", f / 1000.0)
                                        } else {
                                            format!("{:.0}", f)
                                        }
                                    }),
                                chart_width,
                                &theme,
                            )),
                    )
                    .child(
                        div()
                            .ml(px(60.0))
                            .mt_2()
                            .text_sm()
                            .text_color(rgb(0x666666))
                            .text_center()
                            .child("Frequency (Hz)"),
                    ),
            )
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
                let color = colors.get(name).cloned().unwrap_or(D3Color::rgb(128, 128, 128));
                let (r, g, b) = ((color.r * 255.0) as u32, (color.g * 255.0) as u32, (color.b * 255.0) as u32);

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
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x333333))
                            .child(name),
                    )
            }))
    }

    fn render_directivity_plot(&self, plane: &str) -> Div {
        let theme = DefaultAxisTheme;

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
            return div()
                .flex()
                .items_center()
                .justify_center()
                .h_full()
                .child(
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
            return div()
                .flex()
                .items_center()
                .justify_center()
                .h_full()
                .child(
                    div()
                        .text_base()
                        .text_color(rgb(0x666666))
                        .child(format!("No {} directivity data available.", plane)),
                );
        }

        // Create scales
        let freq_scale = LogScale::new().domain(20.0, 20000.0).range(0.0, 800.0);
        let spl_scale = LinearScale::new().domain(-40.0, 10.0).range(0.0, 400.0);

        let chart_width = 800.0;
        let chart_height = 400.0;

        // Generate colors for different angles
        let num_curves = curves.len();

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
                        if plane == "horizontal" { "Horizontal" } else { "Vertical" },
                        self.selected_speaker.as_deref().unwrap_or("Unknown")
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .child(render_axis(
                                &spl_scale,
                                &AxisConfig::left()
                                    .with_ticks(10)
                                    .with_formatter(|v| format!("{:.0} dB", v)),
                                chart_height,
                                &theme,
                            ))
                            .child(
                                div()
                                    .w(px(chart_width as f32))
                                    .h(px(chart_height as f32))
                                    .relative()
                                    .bg(rgb(0xf8f8f8))
                                    .border_1()
                                    .border_color(rgb(0xcccccc))
                                    .child(render_grid(
                                        &freq_scale,
                                        &spl_scale,
                                        &GridConfig::with_lines(),
                                        chart_width,
                                        chart_height,
                                        &theme,
                                    ))
                                    .children(curves.iter().enumerate().map(|(i, curve)| {
                                        let t = i as f32 / (num_curves.max(1) - 1).max(1) as f32;
                                        // Interpolate through viridis colors
                                        let color = d3rs::color::interpolate_colors(&viridis_colors, t);

                                        let points: Vec<LinePoint> = curve
                                            .freq
                                            .iter()
                                            .zip(curve.spl.iter())
                                            .filter(|(&f, _)| f >= 20.0 && f <= 20000.0)
                                            .map(|(&f, &spl)| LinePoint::new(f, spl))
                                            .collect();

                                        render_line(
                                            &freq_scale,
                                            &spl_scale,
                                            &points,
                                            &LineConfig::new()
                                                .stroke_color(color)
                                                .stroke_width(1.5)
                                                .curve(CurveType::Linear),
                                        )
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .ml(px(60.0))
                            .child(render_axis(
                                &freq_scale,
                                &AxisConfig::bottom()
                                    .with_ticks(10)
                                    .with_formatter(|f| {
                                        if f >= 1000.0 {
                                            format!("{:.0}k", f / 1000.0)
                                        } else {
                                            format!("{:.0}", f)
                                        }
                                    }),
                                chart_width,
                                &theme,
                            )),
                    )
                    .child(
                        div()
                            .ml(px(60.0))
                            .mt_2()
                            .text_sm()
                            .text_color(rgb(0x666666))
                            .text_center()
                            .child("Frequency (Hz)"),
                    ),
            )
            // Angle legend
            .child({
                let angle_min = curves.first().map(|c| c.angle).unwrap_or(-60.0);
                let angle_max = curves.last().map(|c| c.angle).unwrap_or(60.0);

                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_4()
                    .bg(rgb(0xf5f5f5))
                    .rounded_md()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x666666))
                            .child(format!("{:.0}°", angle_min)),
                    )
                    // Simplified gradient legend (using color strip segments)
                    .children((0..6).map(|i| {
                        let color = d3rs::color::interpolate_colors(&viridis_colors, i as f32 / 5.0);
                        let (r, g, b) = ((color.r * 255.0) as u32, (color.g * 255.0) as u32, (color.b * 255.0) as u32);
                        div()
                            .flex_1()
                            .h(px(16.0))
                            .bg(rgb((r << 16) | (g << 8) | b))
                    }))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x666666))
                            .child(format!("{:.0}°", angle_max)),
                    )
            })
    }

    fn render_contour_plot(&self) -> Div {
        // Contour plot placeholder - will implement using d3rs contour module
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
                    .text_color(rgb(0x333333))
                    .child("Contour Plot"),
            )
            .child(
                div()
                    .text_base()
                    .text_color(rgb(0x666666))
                    .child("Contour visualization coming soon..."),
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

fn main() {
    Application::new().run(|cx| {
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
