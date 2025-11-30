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
