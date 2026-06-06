//! Layout Builder Showcase
//!
//! Interactive demo of the gpui-builder constraint solver rendered live in GPUI.
//! Demonstrates a 3-panel app layout with:
//! - Fixed header and footer (hard constraints)
//! - Collapsible sidebar and inspector (soft constraints)
//! - Auto-axis switching (resize window to portrait to see vertical stacking)
//! - Display tiers on the inspector panel (Full/Mini based on size)
//! - Draggable dividers to resize panels
//! - Real-time solver output in footer
//!
//! Run: cargo run -p gpui-builder --features showcase --bin layout-showcase

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_builder::types::LayoutPreferences;
use gpui_builder::{
    Axis, ContainerNode, DisplayTier, LayoutNode, Sizing, SlotNode, SolvedNode, solve,
};
use gpui_design::{DesignExt, DesignSystemState};
use std::rc::Rc;

// ============================================================================
// Display tiers
// ============================================================================

static INSPECTOR_TIERS: &[DisplayTier<'_>] = &[
    DisplayTier {
        name: "Full",
        min_size: 200.0,
    },
    DisplayTier {
        name: "Mini",
        min_size: 80.0,
    },
];

// ============================================================================
// View state
// ============================================================================

struct ShowcaseView {
    sidebar_ratio_h: f32,
    sidebar_ratio_v: f32,
    inspector_ratio_h: f32,
    inspector_ratio_v: f32,
    sidebar_collapsed: bool,
    inspector_collapsed: bool,
    dragging: Option<DragTarget>,
}

#[derive(Clone, Copy)]
struct ShowcaseTheme {
    background: Rgba,
    surface: Rgba,
    muted: Rgba,
    border: Rgba,
    accent: Rgba,
    text_primary: Rgba,
    text_muted: Rgba,
}

impl ShowcaseTheme {
    fn dark() -> Self {
        Self {
            background: rgb(0x181818),
            surface: rgb(0x242424),
            muted: rgb(0x2d2d2d),
            border: rgb(0x3a3a3a),
            accent: rgb(0x0a84ff),
            text_primary: rgb(0xf2f2f2),
            text_muted: rgb(0x9a9a9a),
        }
    }
}

#[derive(Clone, Copy)]
enum DragTarget {
    Sidebar,
    Inspector,
}

impl ShowcaseView {
    fn new() -> Self {
        Self {
            sidebar_ratio_h: 0.22,
            sidebar_ratio_v: 0.25,
            inspector_ratio_h: 0.25,
            inspector_ratio_v: 0.20,
            sidebar_collapsed: false,
            inspector_collapsed: false,
            dragging: None,
        }
    }

    fn solve_at(&self, w: f32, h: f32) -> SolvedNode {
        let content_children: &[LayoutNode<'_>] = &[
            LayoutNode::Slot(SlotNode {
                id: "sidebar",
                sizing: Sizing::fractional(0.22, 80.0),
                priority: 0.5,
                collapsible: true,
                display_tiers: &[],
                collapse_label: Some("Sidebar"),
            }),
            LayoutNode::Slot(SlotNode {
                id: "main",
                sizing: Sizing::flex(200.0),
                priority: 1.0,
                collapsible: false,
                display_tiers: &[],
                collapse_label: None,
            }),
            LayoutNode::Slot(SlotNode {
                id: "inspector",
                sizing: Sizing::fractional(0.25, 0.0),
                priority: 0.3,
                collapsible: true,
                display_tiers: INSPECTOR_TIERS,
                collapse_label: Some("Inspector"),
            }),
        ];

        let root_children: &[LayoutNode<'_>] = &[
            LayoutNode::Slot(SlotNode {
                id: "header",
                sizing: Sizing::Fixed(44.0),
                priority: 1.0,
                collapsible: false,
                display_tiers: &[],
                collapse_label: None,
            }),
            LayoutNode::Container(ContainerNode {
                id: "content",
                axis: Axis::Horizontal,
                auto_axis: Some(1.0),
                sizing: Sizing::flex(0.0),
                children: content_children,
                divider_size: 6.0,
            }),
            LayoutNode::Slot(SlotNode {
                id: "footer",
                sizing: Sizing::Fixed(32.0),
                priority: 1.0,
                collapsible: false,
                display_tiers: &[],
                collapse_label: None,
            }),
        ];

        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Vertical,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: root_children,
            divider_size: 0.0,
        });

        let ratios = [
            ("sidebar", Axis::Horizontal, self.sidebar_ratio_h),
            ("sidebar", Axis::Vertical, self.sidebar_ratio_v),
            ("inspector", Axis::Horizontal, self.inspector_ratio_h),
            ("inspector", Axis::Vertical, self.inspector_ratio_v),
        ];
        let collapsed = [
            ("sidebar", self.sidebar_collapsed),
            ("inspector", self.inspector_collapsed),
        ];
        let prefs = LayoutPreferences {
            ratios: &ratios,
            collapsed: &collapsed,
        };

        solve(&root, w, h, &prefs)
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn muted(color: Rgba, alpha: f32) -> Rgba {
    Rgba {
        r: color.r,
        g: color.g,
        b: color.b,
        a: alpha,
    }
}

fn panel_box(
    label: &str,
    size_info: &str,
    bg: Rgba,
    fg: Rgba,
    base_size: f32,
    small_size: f32,
    gap: f32,
) -> impl IntoElement {
    div()
        .size_full()
        .bg(bg)
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(gap))
        .child(
            div()
                .text_size(px(base_size))
                .font_weight(FontWeight::BOLD)
                .text_color(fg)
                .child(SharedString::from(label.to_string())),
        )
        .child(
            div()
                .text_size(px(small_size))
                .text_color(muted(fg, 0.5))
                .child(SharedString::from(size_info.to_string())),
        )
}

fn size_label(node: &SolvedNode) -> String {
    let tier = node
        .active_tier
        .as_deref()
        .map(|t| format!(" [{t}]"))
        .unwrap_or_default();
    format!("{:.0} x {:.0}{tier}", node.width, node.height)
}

// ============================================================================
// Render
// ============================================================================

impl Render for ShowcaseView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = ShowcaseTheme::dark();
        let ds = cx.design();
        let bounds = window.bounds();
        let w: f32 = bounds.size.width.into();
        let h: f32 = bounds.size.height.into();

        let solved = self.solve_at(w, h);

        let content = solved.find("content").unwrap();
        let is_h = content.resolved_axis == Some(Axis::Horizontal);
        let header_h = solved.find("header").unwrap().height;
        let footer_h = solved.find("footer").unwrap().height;
        let tabs = solved.collapsed_tabs();

        let sidebar = solved.find("sidebar").unwrap().clone();
        let main_n = solved.find("main").unwrap().clone();
        let inspector = solved.find("inspector").unwrap().clone();
        let content_w = content.width;
        let content_h = content.height;

        // Colors — tinted variants of surface to distinguish panels
        let header_bg = theme.surface;
        let footer_bg = theme.surface;
        let sidebar_bg = theme.muted;
        let main_bg = theme.background;
        let inspector_bg = theme.muted;
        let divider_color = theme.border;
        let accent = theme.accent;
        let fg = theme.text_primary;
        let base_sz = ds.typography.base_size;
        let small_sz = ds.typography.small_size;

        let axis_label = if is_h { "Horizontal" } else { "Vertical" };
        let sidebar_pct = if is_h {
            self.sidebar_ratio_h
        } else {
            self.sidebar_ratio_v
        } * 100.0;
        let inspector_pct = if is_h {
            self.inspector_ratio_h
        } else {
            self.inspector_ratio_v
        } * 100.0;

        div()
            .id("showcase-root")
            .size_full()
            .bg(theme.background)
            .text_color(fg)
            .flex()
            .flex_col()
            // ---- Header ----
            .child(
                div()
                    .h(px(header_h))
                    .w_full()
                    .bg(header_bg)
                    .flex()
                    .flex_row()
                    .items_center()
                    .px(px(ds.spacing.card_padding))
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(ds.spacing.control_gap + ds.spacing.grid_unit))
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(base_sz))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(accent)
                                    .child("gpui-builder"),
                            )
                            .child(
                                div()
                                    .text_size(px(small_sz))
                                    .text_color(theme.text_muted)
                                    .child(SharedString::from(format!(
                                        "{w:.0}x{h:.0}  {axis_label}"
                                    ))),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(small_sz))
                            .text_color(theme.text_muted)
                            .child("drag dividers | click to collapse | resize window"),
                    ),
            )
            // ---- Content ----
            .child(self.render_content(
                is_h,
                content_w,
                content_h,
                &sidebar,
                &main_n,
                &inspector,
                sidebar_bg,
                main_bg,
                inspector_bg,
                divider_color,
                accent,
                fg,
                &tabs,
                &theme,
                base_sz,
                small_sz,
                ds.typography.large_size,
                cx,
            ))
            // ---- Footer ----
            .child(
                div()
                    .h(px(footer_h))
                    .w_full()
                    .bg(footer_bg)
                    .flex()
                    .flex_row()
                    .items_center()
                    .px(px(ds.spacing.card_padding))
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(small_sz))
                            .text_color(theme.text_muted)
                            .child(SharedString::from(format!(
                                "sidebar: {sidebar_pct:.0}%  inspector: {inspector_pct:.0}%"
                            ))),
                    )
                    .child(if !tabs.is_empty() {
                        let labels: Vec<&str> = tabs.iter().map(|(_, l)| *l).collect();
                        div()
                            .text_size(px(small_sz))
                            .text_color(accent)
                            .child(SharedString::from(format!("Tabs: {}", labels.join(", "))))
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
    }
}

impl ShowcaseView {
    #[allow(clippy::too_many_arguments)]
    fn render_content(
        &self,
        is_h: bool,
        _content_w: f32,
        content_h: f32,
        sidebar: &SolvedNode,
        main_n: &SolvedNode,
        inspector: &SolvedNode,
        sidebar_bg: Rgba,
        main_bg: Rgba,
        inspector_bg: Rgba,
        divider_color: Rgba,
        accent: Rgba,
        fg: Rgba,
        tabs: &[(&str, &str)],
        theme: &ShowcaseTheme,
        base_sz: f32,
        small_sz: f32,
        large_sz: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ds = cx.design();
        // Shared mouse handlers for divider dragging
        let base = div()
            .id("content-area")
            .overflow_hidden()
            .on_mouse_move(
                cx.listener(move |view, event: &MouseMoveEvent, window, _cx| {
                    let Some(target) = view.dragging else { return };
                    let ws = window.bounds().size;
                    let mx: f32 = event.position.x.into();
                    let my: f32 = event.position.y.into();
                    let ww: f32 = ws.width.into();
                    let wh: f32 = ws.height.into();
                    if ww > 0.0 && wh > 0.0 {
                        match (target, is_h) {
                            (DragTarget::Sidebar, true) => {
                                view.sidebar_ratio_h = (mx / ww).clamp(0.08, 0.45)
                            }
                            (DragTarget::Sidebar, false) => {
                                view.sidebar_ratio_v = (my / wh).clamp(0.08, 0.45)
                            }
                            (DragTarget::Inspector, true) => {
                                view.inspector_ratio_h = (1.0 - mx / ww).clamp(0.08, 0.45)
                            }
                            (DragTarget::Inspector, false) => {
                                view.inspector_ratio_v = (1.0 - my / wh).clamp(0.08, 0.45)
                            }
                        }
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _: &MouseUpEvent, _, _| {
                    view.dragging = None;
                }),
            );

        if is_h {
            base.h(px(content_h))
                .w_full()
                .flex()
                .flex_row()
                // Sidebar
                .when(sidebar.visible, |d: Stateful<Div>| {
                    d.child(
                        div()
                            .w(px(sidebar.width))
                            .h_full()
                            .overflow_hidden()
                            .child(panel_box(
                                "Sidebar",
                                &size_label(sidebar),
                                sidebar_bg,
                                fg,
                                base_sz,
                                small_sz,
                                ds.spacing.grid_unit,
                            )),
                    )
                })
                // Sidebar divider
                .child(self.divider_v("sidebar", divider_color, accent, cx))
                // Main
                .child(div().flex_1().h_full().overflow_hidden().child(
                    self.main_panel(main_n, main_bg, fg, tabs, theme, &ds, large_sz, small_sz),
                ))
                // Inspector divider + panel
                .when(inspector.visible, |d: Stateful<Div>| {
                    d.child(self.divider_v("inspector", divider_color, accent, cx))
                        .child(
                            div()
                                .w(px(inspector.width))
                                .h_full()
                                .overflow_hidden()
                                .child(panel_box(
                                    "Inspector",
                                    &size_label(inspector),
                                    inspector_bg,
                                    fg,
                                    base_sz,
                                    small_sz,
                                    ds.spacing.grid_unit,
                                )),
                        )
                })
                .into_any_element()
        } else {
            base.h(px(content_h))
                .w_full()
                .flex()
                .flex_col()
                .when(sidebar.visible, |d: Stateful<Div>| {
                    d.child(
                        div()
                            .h(px(sidebar.height))
                            .w_full()
                            .overflow_hidden()
                            .child(panel_box(
                                "Sidebar",
                                &size_label(sidebar),
                                sidebar_bg,
                                fg,
                                base_sz,
                                small_sz,
                                ds.spacing.grid_unit,
                            )),
                    )
                })
                .child(self.divider_h("sidebar", divider_color, accent, cx))
                .child(div().flex_1().w_full().overflow_hidden().child(
                    self.main_panel(main_n, main_bg, fg, tabs, theme, &ds, large_sz, small_sz),
                ))
                .when(inspector.visible, |d: Stateful<Div>| {
                    d.child(self.divider_h("inspector", divider_color, accent, cx))
                        .child(
                            div()
                                .h(px(inspector.height))
                                .w_full()
                                .overflow_hidden()
                                .child(panel_box(
                                    "Inspector",
                                    &size_label(inspector),
                                    inspector_bg,
                                    fg,
                                    base_sz,
                                    small_sz,
                                    ds.spacing.grid_unit,
                                )),
                        )
                })
                .into_any_element()
        }
    }

    fn main_panel(
        &self,
        node: &SolvedNode,
        bg: Rgba,
        fg: Rgba,
        tabs: &[(&str, &str)],
        theme: &ShowcaseTheme,
        ds: &gpui_design::DesignSystem,
        large_sz: f32,
        small_sz: f32,
    ) -> impl IntoElement {
        let mut el = div()
            .size_full()
            .bg(bg)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(ds.spacing.control_gap))
            .child(
                div()
                    .text_size(px(large_sz))
                    .font_weight(FontWeight::BOLD)
                    .text_color(fg)
                    .child("Main Content"),
            )
            .child(
                div()
                    .text_size(px(small_sz))
                    .text_color(muted(fg, 0.5))
                    .child(SharedString::from(format!(
                        "{:.0} x {:.0}",
                        node.width, node.height
                    ))),
            );

        if !tabs.is_empty() {
            el = el.child(
                div()
                    .mt(px(ds.spacing.section_gap))
                    .flex()
                    .flex_row()
                    .gap(px(ds.spacing.control_gap))
                    .children(tabs.iter().map(|(_, label)| {
                        div()
                            .px(px(ds.spacing.control_padding_x))
                            .py(px(ds.spacing.control_padding_y * 0.5))
                            .rounded(px(ds.corners.md))
                            .bg(muted(theme.accent, 0.15))
                            .border_1()
                            .border_color(muted(theme.accent, 0.3))
                            .text_size(px(small_sz))
                            .text_color(theme.accent)
                            .child(SharedString::from(label.to_string()))
                    })),
            );
        }

        el
    }

    fn divider_v(
        &self,
        panel: &str,
        bg: Rgba,
        hover_bg: Rgba,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = SharedString::from(format!("div-v-{panel}"));
        let is_sidebar = panel == "sidebar";
        let target = if is_sidebar {
            DragTarget::Sidebar
        } else {
            DragTarget::Inspector
        };
        let panel_owned = panel.to_string();

        div()
            .id(id)
            .w(px(6.0))
            .h_full()
            .flex_shrink_0()
            .bg(bg)
            .hover(move |s| s.bg(hover_bg))
            .cursor_col_resize()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseDownEvent, _, _| {
                    view.dragging = Some(target);
                }),
            )
            .on_click(cx.listener(move |view, _: &ClickEvent, _, _| {
                if panel_owned == "sidebar" {
                    view.sidebar_collapsed = !view.sidebar_collapsed;
                } else {
                    view.inspector_collapsed = !view.inspector_collapsed;
                }
            }))
    }

    fn divider_h(
        &self,
        panel: &str,
        bg: Rgba,
        hover_bg: Rgba,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = SharedString::from(format!("div-h-{panel}"));
        let is_sidebar = panel == "sidebar";
        let target = if is_sidebar {
            DragTarget::Sidebar
        } else {
            DragTarget::Inspector
        };
        let panel_owned = panel.to_string();

        div()
            .id(id)
            .h(px(6.0))
            .w_full()
            .flex_shrink_0()
            .bg(bg)
            .hover(move |s| s.bg(hover_bg))
            .cursor_row_resize()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, _: &MouseDownEvent, _, _| {
                    view.dragging = Some(target);
                }),
            )
            .on_click(cx.listener(move |view, _: &ClickEvent, _, _| {
                if panel_owned == "sidebar" {
                    view.sidebar_collapsed = !view.sidebar_collapsed;
                } else {
                    view.inspector_collapsed = !view.inspector_collapsed;
                }
            }))
    }
}

// ============================================================================
// Entry point
// ============================================================================

fn current_platform() -> Result<Rc<dyn gpui::Platform>, String> {
    #[cfg(target_os = "macos")]
    {
        Ok(Rc::new(gpui_macos::MacPlatform::new(false)))
    }
    #[cfg(target_os = "linux")]
    {
        Ok(gpui_linux::current_platform(false))
    }
    #[cfg(target_os = "windows")]
    {
        gpui_windows::WindowsPlatform::new(false)
            .map(|p| Rc::new(p) as Rc<dyn gpui::Platform>)
            .map_err(|e| format!("failed to create Windows platform: {e:?}"))
    }
    #[cfg(any(target_os = "ios", target_os = "tvos"))]
    {
        Ok(gpui_ios::current_platform(false))
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "linux",
        target_os = "windows",
        target_os = "ios",
        target_os = "tvos"
    )))]
    {
        compile_error!("unsupported platform for layout showcase")
    }
}

fn main() {
    let platform = match current_platform() {
        Ok(platform) => platform,
        Err(error) => {
            eprintln!("Layout showcase platform error: {error}");
            return;
        }
    };

    gpui::Application::with_platform(platform).run(|cx: &mut App| {
        cx.set_global(DesignSystemState::new());

        let bounds = Bounds::centered(None, size(px(1000.0), px(700.0)), cx);
        if let Err(error) = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Layout Builder Showcase".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_cx| ShowcaseView::new()),
        ) {
            eprintln!("Layout showcase window error: {error:?}");
            return;
        }

        cx.activate(true);
    });
}
