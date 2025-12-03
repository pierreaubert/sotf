//! d3rs Showcase - Unified demo application
//!
//! Demonstrates all d3rs functionality in a single application with tabbed navigation.

use gpui::{Menu, MenuItem, *};

mod showcase_modules;

// Demo sections
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DemoSection {
    #[default]
    Overview,
    Scales,
    Axes,
    BarCharts,
    LineCharts,
    ScatterPlots,
    SurfacePlots,
    QuadTree,
    Contours,
    Transitions,
    Geo,
    Colors,
    // D3 Observable Examples
    D3VolcanoContours,
    D3KDE,
    D3Treemap,
    D3StackedBars,
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
            Self::SurfacePlots,
            Self::QuadTree,
            Self::Contours,
            Self::Transitions,
            Self::Geo,
            Self::Colors,
            // D3 Observable Examples
            Self::D3VolcanoContours,
            Self::D3KDE,
            Self::D3Treemap,
            Self::D3StackedBars,
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
            Self::SurfacePlots => "Surface Plots",
            Self::QuadTree => "QuadTree",
            Self::Contours => "Contours",
            Self::Transitions => "Transitions",
            Self::Geo => "Geo",
            Self::Colors => "Colors",
            // D3 Observable Examples
            Self::D3VolcanoContours => "D3: Volcano",
            Self::D3KDE => "D3: KDE",
            Self::D3Treemap => "D3: Treemap",
            Self::D3StackedBars => "D3: Stacked Bars",
        }
    }
}

/// Contour rendering mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContourRenderMode {
    #[default]
    Isoline,
    Surface,
    Heatmap,
}

impl ContourRenderMode {
    fn label(&self) -> &'static str {
        match self {
            Self::Isoline => "Isoline",
            Self::Surface => "Surface",
            Self::Heatmap => "Heatmap",
        }
    }

    fn next(&self) -> Self {
        match self {
            Self::Isoline => Self::Surface,
            Self::Surface => Self::Heatmap,
            Self::Heatmap => Self::Isoline,
        }
    }
}

pub struct ShowcaseApp {
    pub current_section: DemoSection,
    // Contour demo parameters
    pub contour_grid_size: usize,
    pub contour_num_levels: usize,
    pub contour_peak1_x: f32,
    pub contour_peak1_y: f32,
    pub contour_peak2_x: f32,
    pub contour_peak2_y: f32,
    pub density_bandwidth: f32,
    pub density_num_points: usize,
    pub contour_render_mode: ContourRenderMode,
    // QuadTree demo parameters
    pub quadtree_query_x: f32,
    pub quadtree_query_y: f32,
    pub quadtree_search_radius: f32,
    // D3 Volcano Contours example parameters
    pub volcano_num_thresholds: usize,
    pub volcano_color_scale: showcase_modules::d3_examples::volcano_contours::VolcanoColorScale,
    pub volcano_show_stroke: bool,
    // D3 KDE example parameters
    pub kde_bandwidth: f64,
    pub kde_kernel_type: showcase_modules::d3_examples::KernelType,
    pub kde_show_histogram: bool,
    pub kde_bin_count: usize,
    // D3 Treemap example parameters
    pub treemap_tiling: showcase_modules::d3_examples::TilingMethod,
    pub treemap_padding: f32,
    // D3 Stacked/Grouped Bars example parameters
    pub stacked_bars_layout: showcase_modules::d3_examples::BarLayout,
    pub stacked_bars_n_series: usize,
    pub stacked_bars_m_samples: usize,
    pub stacked_bars_animation_progress: f64,
    pub stacked_bars_animating: bool,
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
            contour_render_mode: ContourRenderMode::default(),
            quadtree_query_x: 50.0,
            quadtree_query_y: 50.0,
            quadtree_search_radius: 15.0,
            // D3 Volcano Contours defaults
            volcano_num_thresholds: 20,
            volcano_color_scale:
                showcase_modules::d3_examples::volcano_contours::VolcanoColorScale::default(),
            volcano_show_stroke: false,
            // D3 KDE defaults
            kde_bandwidth: 7.0,
            kde_kernel_type: showcase_modules::d3_examples::KernelType::default(),
            kde_show_histogram: true,
            kde_bin_count: 20,
            // D3 Treemap defaults
            treemap_tiling: showcase_modules::d3_examples::TilingMethod::default(),
            treemap_padding: 1.0,
            // D3 Stacked/Grouped Bars defaults
            stacked_bars_layout: showcase_modules::d3_examples::BarLayout::default(),
            stacked_bars_n_series: 5,
            stacked_bars_m_samples: 40,
            stacked_bars_animation_progress: 0.0,
            stacked_bars_animating: false,
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
            DemoSection::Overview => showcase_modules::overview::render(self),
            DemoSection::Scales => showcase_modules::scales::render(self),
            DemoSection::Axes => showcase_modules::axes::render(self),
            DemoSection::BarCharts => showcase_modules::bar_charts::render(self),
            DemoSection::LineCharts => showcase_modules::line_charts::render(self),
            DemoSection::ScatterPlots => showcase_modules::scatter_plots::render(self),
            DemoSection::SurfacePlots => showcase_modules::surface_plots::render(self, cx),
            DemoSection::QuadTree => showcase_modules::quadtree::render(self, cx),
            DemoSection::Contours => showcase_modules::contours::render(self, cx),
            DemoSection::Transitions => showcase_modules::transitions::render(self),
            DemoSection::Geo => showcase_modules::geo::render(self),
            DemoSection::Colors => showcase_modules::colors::render(self),
            // D3 Observable Examples
            DemoSection::D3VolcanoContours => showcase_modules::d3_examples::render(self, cx),
            DemoSection::D3KDE => {
                showcase_modules::d3_examples::kernel_density_estimation::render(self, cx)
            }
            DemoSection::D3Treemap => showcase_modules::d3_examples::treemap::render(self, cx),
            DemoSection::D3StackedBars => {
                showcase_modules::d3_examples::stacked_grouped_bars::render(self, cx)
            }
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
actions!(showcase_main, [Quit]);

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
