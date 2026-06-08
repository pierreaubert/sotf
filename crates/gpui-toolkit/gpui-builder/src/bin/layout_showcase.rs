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
//! - Visual solved-tree inspector with live node highlighting
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
    dragging: Option<DragSession>,
    drag_moved: bool,
    suppress_next_divider_click: bool,
    selected_node: Option<String>,
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

#[derive(Clone, Copy)]
struct DragSession {
    target: DragTarget,
    axis: Axis,
    start_pos: f32,
    start_ratio: f32,
    extent: f32,
}

impl DragSession {
    fn axis_position(&self, position: Point<Pixels>) -> f32 {
        match self.axis {
            Axis::Horizontal => position.x.into(),
            Axis::Vertical => position.y.into(),
        }
    }
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
            drag_moved: false,
            suppress_next_divider_click: false,
            selected_node: Some("root".to_string()),
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

    fn begin_drag(&mut self, target: DragTarget, axis: Axis, start_pos: f32, extent: f32) {
        let start_ratio = match (target, axis) {
            (DragTarget::Sidebar, Axis::Horizontal) => self.sidebar_ratio_h,
            (DragTarget::Sidebar, Axis::Vertical) => self.sidebar_ratio_v,
            (DragTarget::Inspector, Axis::Horizontal) => self.inspector_ratio_h,
            (DragTarget::Inspector, Axis::Vertical) => self.inspector_ratio_v,
        };

        self.dragging = Some(DragSession {
            target,
            axis,
            start_pos,
            start_ratio,
            extent: extent.max(1.0),
        });
        self.drag_moved = false;
        self.suppress_next_divider_click = false;
    }

    fn update_drag_from_position(&mut self, position: Point<Pixels>) -> bool {
        let Some(drag) = self.dragging else {
            return false;
        };
        let delta = (drag.axis_position(position) - drag.start_pos) / drag.extent;
        let next = match drag.target {
            DragTarget::Sidebar => drag.start_ratio + delta,
            DragTarget::Inspector => drag.start_ratio - delta,
        }
        .clamp(0.08, 0.45);

        let ratio = match (drag.target, drag.axis) {
            (DragTarget::Sidebar, Axis::Horizontal) => &mut self.sidebar_ratio_h,
            (DragTarget::Sidebar, Axis::Vertical) => &mut self.sidebar_ratio_v,
            (DragTarget::Inspector, Axis::Horizontal) => &mut self.inspector_ratio_h,
            (DragTarget::Inspector, Axis::Vertical) => &mut self.inspector_ratio_v,
        };

        if (*ratio - next).abs() <= 0.001 {
            return false;
        }

        *ratio = next;
        self.drag_moved = true;
        true
    }

    fn finish_drag(&mut self, position: Point<Pixels>) -> bool {
        let Some(drag) = self.dragging.take() else {
            return false;
        };
        let moved = self.drag_moved || (drag.axis_position(position) - drag.start_pos).abs() > 3.0;
        self.suppress_next_divider_click = moved;
        self.drag_moved = false;
        true
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
    selected: bool,
    accent: Rgba,
    base_size: f32,
    small_size: f32,
    gap: f32,
) -> impl IntoElement {
    div()
        .size_full()
        .min_w_0()
        .min_h_0()
        .bg(bg)
        .when(selected, |d| d.border_1().border_color(muted(accent, 0.8)))
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

#[derive(Debug)]
struct VisualTreeRow {
    id: String,
    depth: usize,
    width: f32,
    height: f32,
    visible: bool,
    resolved_axis: Option<Axis>,
    active_tier: Option<String>,
}

fn collect_visual_tree_rows(node: &SolvedNode, depth: usize, rows: &mut Vec<VisualTreeRow>) {
    rows.push(VisualTreeRow {
        id: node.id.clone(),
        depth,
        width: node.width,
        height: node.height,
        visible: node.visible,
        resolved_axis: node.resolved_axis,
        active_tier: node.active_tier.clone(),
    });

    for child in &node.children {
        collect_visual_tree_rows(child, depth + 1, rows);
    }
}

fn axis_text(axis: Option<Axis>) -> &'static str {
    match axis {
        Some(Axis::Horizontal) => "H",
        Some(Axis::Vertical) => "V",
        None => "-",
    }
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
        let selected_id = self.selected_node.as_deref().unwrap_or("root");

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
            .when(selected_id == "root", |d| {
                d.border_1().border_color(muted(accent, 0.8))
            })
            .on_mouse_move(
                cx.listener(move |view, event: &MouseMoveEvent, _window, cx| {
                    if view.update_drag_from_position(event.position) {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, event: &MouseUpEvent, _, cx| {
                    let changed = view.update_drag_from_position(event.position);
                    let finished = view.finish_drag(event.position);
                    if changed || finished {
                        cx.notify();
                    }
                }),
            )
            // ---- Header ----
            .child(
                div()
                    .h(px(header_h))
                    .w_full()
                    .bg(header_bg)
                    .when(selected_id == "header", |d| {
                        d.border_1().border_color(muted(accent, 0.8))
                    })
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
                            .child("click tree rows | drag dividers | resize window"),
                    ),
            )
            // ---- Content ----
            .child(self.render_content(
                &solved,
                selected_id,
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
                    .when(selected_id == "footer", |d| {
                        d.border_1().border_color(muted(accent, 0.8))
                    })
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
        solved: &SolvedNode,
        selected_id: &str,
        is_h: bool,
        content_w: f32,
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
        let base = div()
            .id("content-area")
            .overflow_hidden()
            .min_w_0()
            .min_h_0()
            .when(selected_id == "content", |d| {
                d.border_1().border_color(muted(accent, 0.8))
            });

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
                            .min_w_0()
                            .overflow_hidden()
                            .when(selected_id == "sidebar", |d| {
                                d.border_1().border_color(muted(accent, 0.8))
                            })
                            .child(panel_box(
                                "Sidebar",
                                &size_label(sidebar),
                                sidebar_bg,
                                fg,
                                false,
                                accent,
                                base_sz,
                                small_sz,
                                ds.spacing.grid_unit,
                            )),
                    )
                })
                // Sidebar divider
                .child(self.divider_v("sidebar", divider_color, accent, content_w, cx))
                // Main
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .min_w_0()
                        .overflow_hidden()
                        .child(self.main_panel(
                            main_n,
                            main_bg,
                            fg,
                            tabs,
                            theme,
                            &ds,
                            selected_id == "main",
                            large_sz,
                            small_sz,
                        )),
                )
                // Inspector divider + panel
                .when(inspector.visible, |d: Stateful<Div>| {
                    d.child(self.divider_v("inspector", divider_color, accent, content_w, cx))
                        .child(
                            div()
                                .w(px(inspector.width))
                                .h_full()
                                .min_w_0()
                                .overflow_hidden()
                                .when(selected_id == "inspector", |d| {
                                    d.border_1().border_color(muted(accent, 0.8))
                                })
                                .child(self.visual_tree_inspector(
                                    solved,
                                    inspector,
                                    selected_id,
                                    inspector_bg,
                                    fg,
                                    theme,
                                    &ds,
                                    base_sz,
                                    small_sz,
                                    cx,
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
                            .min_h_0()
                            .overflow_hidden()
                            .when(selected_id == "sidebar", |d| {
                                d.border_1().border_color(muted(accent, 0.8))
                            })
                            .child(panel_box(
                                "Sidebar",
                                &size_label(sidebar),
                                sidebar_bg,
                                fg,
                                false,
                                accent,
                                base_sz,
                                small_sz,
                                ds.spacing.grid_unit,
                            )),
                    )
                })
                .child(self.divider_h("sidebar", divider_color, accent, content_h, cx))
                .child(
                    div()
                        .flex_1()
                        .w_full()
                        .min_h_0()
                        .overflow_hidden()
                        .child(self.main_panel(
                            main_n,
                            main_bg,
                            fg,
                            tabs,
                            theme,
                            &ds,
                            selected_id == "main",
                            large_sz,
                            small_sz,
                        )),
                )
                .when(inspector.visible, |d: Stateful<Div>| {
                    d.child(self.divider_h("inspector", divider_color, accent, content_h, cx))
                        .child(
                            div()
                                .h(px(inspector.height))
                                .w_full()
                                .min_h_0()
                                .overflow_hidden()
                                .when(selected_id == "inspector", |d| {
                                    d.border_1().border_color(muted(accent, 0.8))
                                })
                                .child(self.visual_tree_inspector(
                                    solved,
                                    inspector,
                                    selected_id,
                                    inspector_bg,
                                    fg,
                                    theme,
                                    &ds,
                                    base_sz,
                                    small_sz,
                                    cx,
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
        selected: bool,
        large_sz: f32,
        small_sz: f32,
    ) -> impl IntoElement {
        let mut el = div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(bg)
            .when(selected, |d| {
                d.border_1().border_color(muted(theme.accent, 0.8))
            })
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

    #[allow(clippy::too_many_arguments)]
    fn visual_tree_inspector(
        &self,
        solved: &SolvedNode,
        panel: &SolvedNode,
        selected_id: &str,
        bg: Rgba,
        fg: Rgba,
        theme: &ShowcaseTheme,
        ds: &gpui_design::DesignSystem,
        base_sz: f32,
        small_sz: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut rows = Vec::new();
        collect_visual_tree_rows(solved, 0, &mut rows);
        let node_count = rows.len();
        let selected_size = solved.find(selected_id).map(size_label).unwrap_or_default();
        let tree_rows: Vec<AnyElement> = rows
            .into_iter()
            .map(|row| {
                self.visual_tree_row(row, selected_id, theme, ds, small_sz, cx)
                    .into_any_element()
            })
            .collect();

        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(bg)
            .flex()
            .flex_col()
            .gap(px(ds.spacing.control_gap))
            .p(px(ds.spacing.control_gap))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(base_sz))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(fg)
                                    .child("Visual Tree"),
                            )
                            .child(
                                div()
                                    .text_size(px(small_sz))
                                    .text_color(theme.text_muted)
                                    .child(SharedString::from(format!(
                                        "{node_count} solved nodes"
                                    ))),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(small_sz))
                            .text_color(muted(theme.accent, 0.9))
                            .child(SharedString::from(size_label(panel))),
                    ),
            )
            .child(
                div()
                    .rounded(px(ds.corners.sm))
                    .border_1()
                    .border_color(theme.border)
                    .bg(muted(theme.background, 0.55))
                    .px(px(ds.spacing.control_gap))
                    .py(px(ds.spacing.control_padding_y * 0.75))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(small_sz))
                            .text_color(theme.text_muted)
                            .child("Selected"),
                    )
                    .child(div().text_size(px(base_sz)).text_color(theme.accent).child(
                        SharedString::from(format!("{selected_id}  {selected_size}")),
                    )),
            )
            .child(
                div()
                    .id("visual-tree-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .children(tree_rows),
            )
    }

    fn visual_tree_row(
        &self,
        row: VisualTreeRow,
        selected_id: &str,
        theme: &ShowcaseTheme,
        ds: &gpui_design::DesignSystem,
        small_sz: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = row.id == selected_id;
        let row_id = row.id.clone();
        let label = if row.visible {
            row.id.clone()
        } else {
            format!("{} (collapsed)", row.id)
        };
        let meta = format!(
            "{}x{}  axis={}{}",
            row.width.round(),
            row.height.round(),
            axis_text(row.resolved_axis),
            row.active_tier
                .as_deref()
                .map(|tier| format!("  tier={tier}"))
                .unwrap_or_default()
        );
        let indent = row.depth as f32 * 14.0;

        div()
            .id(SharedString::from(format!("tree-row-{}", row.id)))
            .rounded(px(ds.corners.sm))
            .px(px(ds.spacing.control_padding_x * 0.75))
            .py(px(ds.spacing.control_padding_y * 0.65))
            .bg(if is_selected {
                muted(theme.accent, 0.18)
            } else {
                rgba(0x00000000)
            })
            .border_1()
            .border_color(if is_selected {
                muted(theme.accent, 0.55)
            } else {
                rgba(0x00000000)
            })
            .hover(|s| {
                s.bg(muted(theme.accent, 0.10))
                    .border_color(muted(theme.accent, 0.25))
                    .cursor_pointer()
            })
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                view.selected_node = Some(row_id.clone());
                cx.notify();
            }))
            .child(
                div()
                    .ml(px(indent))
                    .flex()
                    .flex_col()
                    .gap(px(1.0))
                    .child(
                        div()
                            .text_size(px(small_sz))
                            .text_color(if row.visible {
                                theme.text_primary
                            } else {
                                theme.text_muted
                            })
                            .child(SharedString::from(label)),
                    )
                    .child(
                        div()
                            .text_size(px((small_sz - 1.0).max(10.0)))
                            .text_color(theme.text_muted)
                            .child(SharedString::from(meta)),
                    ),
            )
    }

    fn divider_v(
        &self,
        panel: &str,
        bg: Rgba,
        hover_bg: Rgba,
        drag_extent: f32,
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
                cx.listener(move |view, event: &MouseDownEvent, _, cx| {
                    let start_pos: f32 = event.position.x.into();
                    view.begin_drag(target, Axis::Horizontal, start_pos, drag_extent);
                    cx.notify();
                }),
            )
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                if view.suppress_next_divider_click {
                    view.suppress_next_divider_click = false;
                    cx.notify();
                    return;
                }
                if panel_owned == "sidebar" {
                    view.sidebar_collapsed = !view.sidebar_collapsed;
                } else {
                    view.inspector_collapsed = !view.inspector_collapsed;
                }
                cx.notify();
            }))
    }

    fn divider_h(
        &self,
        panel: &str,
        bg: Rgba,
        hover_bg: Rgba,
        drag_extent: f32,
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
                cx.listener(move |view, event: &MouseDownEvent, _, cx| {
                    let start_pos: f32 = event.position.y.into();
                    view.begin_drag(target, Axis::Vertical, start_pos, drag_extent);
                    cx.notify();
                }),
            )
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                if view.suppress_next_divider_click {
                    view.suppress_next_divider_click = false;
                    cx.notify();
                    return;
                }
                if panel_owned == "sidebar" {
                    view.sidebar_collapsed = !view.sidebar_collapsed;
                } else {
                    view.inspector_collapsed = !view.inspector_collapsed;
                }
                cx.notify();
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
