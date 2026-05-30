//! Layout inspection records for developer tooling.
//!
//! The core solver stays platform agnostic. This module exposes owned, stable
//! records that debug overlays, showcase apps, and snapshot tests can consume
//! without depending on borrowed declaration lifetimes or solver internals.

use std::fmt;

use crate::{Axis, DisplayTier, LayoutNode, Sizing, SolvedNode};

/// A layout node category that is easy for tooling to switch on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutInspectionKind {
    /// A leaf slot where the application renders content.
    Slot,
    /// A container that arranges children.
    Container,
}

/// Owned sizing metadata for inspection and export.
#[derive(Debug, Clone, PartialEq)]
pub enum SizingInspection {
    /// Fixed pixel size.
    Fixed { size: f32 },
    /// Fractional size with initial ratio and bounds.
    Fractional { initial: f32, min: f32, max: f32 },
    /// Flex size with minimum size and weight.
    Flex { min: f32, weight: f32 },
    /// Text-measured size metadata. The measure implementation is intentionally
    /// not exported because it is not inspectable data.
    Text {
        text: String,
        line_height: f32,
        min: f32,
    },
}

impl SizingInspection {
    /// Return a compact, stable summary.
    pub fn summary(&self) -> String {
        match self {
            Self::Fixed { size } => format!("fixed({})", format_number(*size)),
            Self::Fractional { initial, min, max } => format!(
                "fractional(initial={}, min={}, max={})",
                format_number(*initial),
                format_number(*min),
                format_max(*max)
            ),
            Self::Flex { min, weight } => format!(
                "flex(min={}, weight={})",
                format_number(*min),
                format_number(*weight)
            ),
            Self::Text {
                text,
                line_height,
                min,
            } => format!(
                "text(text={text:?}, line_height={}, min={})",
                format_number(*line_height),
                format_number(*min)
            ),
        }
    }
}

impl fmt::Display for SizingInspection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.summary())
    }
}

/// Display tier metadata copied out of a declaration tree.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayTierInspection {
    /// Tier name.
    pub name: String,
    /// Minimum main-axis size for activation.
    pub min_size: f32,
}

/// Slot-only inspection metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotInspection {
    /// Collapse priority, where lower values collapse first.
    pub priority: f32,
    /// Whether this slot may collapse.
    pub collapsible: bool,
    /// Optional label shown when the slot is collapsed.
    pub collapse_label: Option<String>,
    /// Display tiers declared for the slot.
    pub display_tiers: Vec<DisplayTierInspection>,
}

/// Container-only inspection metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct ContainerInspection {
    /// Declared axis before auto-axis switching.
    pub axis: Axis,
    /// Optional auto-axis aspect threshold.
    pub auto_axis: Option<f32>,
    /// Divider size reserved between visible children.
    pub divider_size: f32,
}

/// One node in an inspected layout declaration tree.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutInspectionNode {
    /// Slash-separated stable path from the inspected root.
    pub path: String,
    /// Node id from the declaration.
    pub id: String,
    /// Slot or container category.
    pub kind: LayoutInspectionKind,
    /// Owned sizing metadata.
    pub sizing: SizingInspection,
    /// Slot metadata when `kind == Slot`.
    pub slot: Option<SlotInspection>,
    /// Container metadata when `kind == Container`.
    pub container: Option<ContainerInspection>,
    /// Inspected children in declaration order.
    pub children: Vec<LayoutInspectionNode>,
}

/// Inspectable declaration tree export.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutInspection {
    /// Inspected root node.
    pub root: LayoutInspectionNode,
}

impl LayoutInspection {
    /// Return all declaration nodes in deterministic depth-first order.
    pub fn nodes(&self) -> Vec<&LayoutInspectionNode> {
        let mut nodes = Vec::new();
        self.root.collect_nodes(&mut nodes);
        nodes
    }

    /// Render a stable line-oriented report for tests and CLI tooling.
    pub fn to_text(&self) -> String {
        let mut output = String::from("layout inspection:\n");
        self.root.write_text(&mut output, 0);
        output
    }
}

impl fmt::Display for LayoutInspection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_text())
    }
}

impl LayoutInspectionNode {
    fn collect_nodes<'a>(&'a self, nodes: &mut Vec<&'a LayoutInspectionNode>) {
        nodes.push(self);
        for child in &self.children {
            child.collect_nodes(nodes);
        }
    }

    fn write_text(&self, output: &mut String, indent: usize) {
        let pad = "  ".repeat(indent);
        match (&self.slot, &self.container) {
            (Some(slot), None) => {
                output.push_str(&format!(
                    "{pad}- {path} slot sizing={sizing} priority={priority} \
                     collapsible={collapsible} collapse_label={label} tiers={tiers}\n",
                    path = self.path,
                    sizing = self.sizing,
                    priority = format_number(slot.priority),
                    collapsible = slot.collapsible,
                    label = option_text(slot.collapse_label.as_deref()),
                    tiers = tiers_text(&slot.display_tiers),
                ));
            }
            (None, Some(container)) => {
                output.push_str(&format!(
                    "{pad}- {path} container sizing={sizing} axis={axis} auto_axis={auto_axis} \
                     divider={divider} children={children}\n",
                    path = self.path,
                    sizing = self.sizing,
                    axis = axis_name(container.axis),
                    auto_axis = option_number(container.auto_axis),
                    divider = format_number(container.divider_size),
                    children = self.children.len(),
                ));
            }
            _ => {
                output.push_str(&format!(
                    "{pad}- {path} unknown sizing={sizing} children={children}\n",
                    path = self.path,
                    sizing = self.sizing,
                    children = self.children.len(),
                ));
            }
        }

        for child in &self.children {
            child.write_text(output, indent + 1);
        }
    }
}

/// One node in an inspected solved layout tree.
#[derive(Debug, Clone, PartialEq)]
pub struct SolvedInspectionNode {
    /// Slash-separated stable path from the solved root.
    pub path: String,
    /// Node id from the solved tree.
    pub id: String,
    /// Resolved width in pixels.
    pub width: f32,
    /// Resolved height in pixels.
    pub height: f32,
    /// Whether this node is visible.
    pub visible: bool,
    /// Active display tier, if any.
    pub active_tier: Option<String>,
    /// Collapse label, if any.
    pub collapse_label: Option<String>,
    /// Resolved axis for containers.
    pub resolved_axis: Option<Axis>,
    /// Inspected children in solved order.
    pub children: Vec<SolvedInspectionNode>,
}

/// Inspectable solved tree export.
#[derive(Debug, Clone, PartialEq)]
pub struct SolvedInspection {
    /// Inspected root node.
    pub root: SolvedInspectionNode,
}

impl SolvedInspection {
    /// Return all solved nodes in deterministic depth-first order.
    pub fn nodes(&self) -> Vec<&SolvedInspectionNode> {
        let mut nodes = Vec::new();
        self.root.collect_nodes(&mut nodes);
        nodes
    }

    /// Render a stable line-oriented report for tests and CLI tooling.
    pub fn to_text(&self) -> String {
        let mut output = String::from("solved inspection:\n");
        self.root.write_text(&mut output, 0);
        output
    }
}

impl fmt::Display for SolvedInspection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_text())
    }
}

impl SolvedInspectionNode {
    fn collect_nodes<'a>(&'a self, nodes: &mut Vec<&'a SolvedInspectionNode>) {
        nodes.push(self);
        for child in &self.children {
            child.collect_nodes(nodes);
        }
    }

    fn write_text(&self, output: &mut String, indent: usize) {
        let pad = "  ".repeat(indent);
        let visible = if self.visible { "visible" } else { "collapsed" };
        output.push_str(&format!(
            "{pad}- {path} size={width}x{height} {visible} axis={axis} tier={tier} \
             collapse_label={label} children={children}\n",
            path = self.path,
            width = format_number(self.width),
            height = format_number(self.height),
            axis = self.resolved_axis.map(axis_name).unwrap_or("-"),
            tier = option_text(self.active_tier.as_deref()),
            label = option_text(self.collapse_label.as_deref()),
            children = self.children.len(),
        ));

        for child in &self.children {
            child.write_text(output, indent + 1);
        }
    }
}

/// Inspect a layout declaration tree.
pub fn inspect_layout(root: &LayoutNode<'_>) -> LayoutInspection {
    LayoutInspection {
        root: inspect_layout_node(root, None),
    }
}

/// Inspect a solved layout tree.
pub fn inspect_solved(root: &SolvedNode) -> SolvedInspection {
    SolvedInspection {
        root: inspect_solved_node(root, None),
    }
}

fn inspect_layout_node(node: &LayoutNode<'_>, parent_path: Option<&str>) -> LayoutInspectionNode {
    let id = node.id();
    let path = node_path(parent_path, id);
    match node {
        LayoutNode::Slot(slot) => LayoutInspectionNode {
            path,
            id: id.to_string(),
            kind: LayoutInspectionKind::Slot,
            sizing: inspect_sizing(slot.sizing),
            slot: Some(SlotInspection {
                priority: slot.priority,
                collapsible: slot.collapsible,
                collapse_label: slot.collapse_label.map(str::to_string),
                display_tiers: inspect_display_tiers(slot.display_tiers),
            }),
            container: None,
            children: Vec::new(),
        },
        LayoutNode::Container(container) => {
            let children = container
                .children
                .iter()
                .map(|child| inspect_layout_node(child, Some(&path)))
                .collect();
            LayoutInspectionNode {
                path,
                id: id.to_string(),
                kind: LayoutInspectionKind::Container,
                sizing: inspect_sizing(container.sizing),
                slot: None,
                container: Some(ContainerInspection {
                    axis: container.axis,
                    auto_axis: container.auto_axis,
                    divider_size: container.divider_size,
                }),
                children,
            }
        }
    }
}

fn inspect_solved_node(node: &SolvedNode, parent_path: Option<&str>) -> SolvedInspectionNode {
    let path = node_path(parent_path, &node.id);
    let children = node
        .children
        .iter()
        .map(|child| inspect_solved_node(child, Some(&path)))
        .collect();
    SolvedInspectionNode {
        path,
        id: node.id.clone(),
        width: node.width,
        height: node.height,
        visible: node.visible,
        active_tier: node.active_tier.clone(),
        collapse_label: node.collapse_label.clone(),
        resolved_axis: node.resolved_axis,
        children,
    }
}

fn inspect_sizing(sizing: Sizing<'_>) -> SizingInspection {
    match sizing {
        Sizing::Fixed(size) => SizingInspection::Fixed { size },
        Sizing::Fractional { initial, min, max } => {
            SizingInspection::Fractional { initial, min, max }
        }
        Sizing::Flex { min, weight } => SizingInspection::Flex { min, weight },
        Sizing::Text {
            text,
            line_height,
            min,
            ..
        } => SizingInspection::Text {
            text: text.to_string(),
            line_height,
            min,
        },
    }
}

fn inspect_display_tiers(tiers: &[DisplayTier<'_>]) -> Vec<DisplayTierInspection> {
    tiers
        .iter()
        .map(|tier| DisplayTierInspection {
            name: tier.name.to_string(),
            min_size: tier.min_size,
        })
        .collect()
}

fn node_path(parent_path: Option<&str>, id: &str) -> String {
    let segment = if id.is_empty() { "<empty>" } else { id };
    match parent_path {
        Some(parent_path) => format!("{parent_path}/{segment}"),
        None => segment.to_string(),
    }
}

fn axis_name(axis: Axis) -> &'static str {
    match axis {
        Axis::Horizontal => "horizontal",
        Axis::Vertical => "vertical",
    }
}

fn option_text(value: Option<&str>) -> &str {
    value.filter(|value| !value.is_empty()).unwrap_or("-")
}

fn option_number(value: Option<f32>) -> String {
    value.map(format_number).unwrap_or_else(|| "-".to_string())
}

fn tiers_text(tiers: &[DisplayTierInspection]) -> String {
    if tiers.is_empty() {
        return "[]".to_string();
    }

    let entries = tiers
        .iter()
        .map(|tier| format!("{}@{}", tier.name, format_number(tier.min_size)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{entries}]")
}

fn format_max(value: f32) -> String {
    if value == f32::MAX {
        "unbounded".to_string()
    } else {
        format_number(value)
    }
}

fn format_number(value: f32) -> String {
    if !value.is_finite() {
        return value.to_string();
    }

    let mut text = format!("{value:.2}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if text == "-0" { "0".to_string() } else { text }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContainerNode, LayoutPreferences, SlotNode, solve};

    static TIERS: &[DisplayTier<'_>] = &[
        DisplayTier {
            name: "Full",
            min_size: 160.0,
        },
        DisplayTier {
            name: "Mini",
            min_size: 80.0,
        },
    ];

    fn sample_tree<'a>(children: &'a [LayoutNode<'a>]) -> LayoutNode<'a> {
        LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: Some(1.0),
            sizing: Sizing::flex(0.0),
            children,
            divider_size: 4.0,
        })
    }

    #[test]
    fn inspects_declared_layout_tree() {
        let children = [
            LayoutNode::Slot(SlotNode {
                id: "sidebar",
                sizing: Sizing::fractional(0.25, 80.0),
                priority: 0.4,
                collapsible: true,
                display_tiers: TIERS,
                collapse_label: Some("Sidebar"),
            }),
            LayoutNode::Slot(SlotNode {
                id: "content",
                sizing: Sizing::flex(120.0),
                priority: 1.0,
                collapsible: false,
                display_tiers: &[],
                collapse_label: None,
            }),
        ];
        let root = sample_tree(&children);

        let inspection = inspect_layout(&root);

        assert_eq!(inspection.nodes().len(), 3);
        assert_eq!(inspection.root.path, "root");
        assert_eq!(inspection.root.kind, LayoutInspectionKind::Container);
        assert_eq!(inspection.root.children[0].path, "root/sidebar");
        assert_eq!(inspection.root.children[0].kind, LayoutInspectionKind::Slot);
        assert_eq!(
            inspection.root.children[0]
                .slot
                .as_ref()
                .unwrap()
                .display_tiers,
            vec![
                DisplayTierInspection {
                    name: "Full".to_string(),
                    min_size: 160.0,
                },
                DisplayTierInspection {
                    name: "Mini".to_string(),
                    min_size: 80.0,
                },
            ]
        );
    }

    #[test]
    fn declared_layout_text_is_stable() {
        let children = [
            LayoutNode::Slot(SlotNode {
                id: "sidebar",
                sizing: Sizing::fractional(0.25, 80.0),
                priority: 0.4,
                collapsible: true,
                display_tiers: TIERS,
                collapse_label: Some("Sidebar"),
            }),
            LayoutNode::Slot(SlotNode {
                id: "content",
                sizing: Sizing::Fixed(120.0),
                priority: 1.0,
                collapsible: false,
                display_tiers: &[],
                collapse_label: None,
            }),
        ];
        let root = sample_tree(&children);

        let inspection = inspect_layout(&root);

        assert_eq!(
            inspection.to_text(),
            concat!(
                "layout inspection:\n",
                "- root container sizing=flex(min=0, weight=1) axis=horizontal ",
                "auto_axis=1 divider=4 children=2\n",
                "  - root/sidebar slot sizing=fractional(initial=0.25, min=80, ",
                "max=unbounded) priority=0.4 collapsible=true collapse_label=Sidebar ",
                "tiers=[Full@160, Mini@80]\n",
                "  - root/content slot sizing=fixed(120) priority=1 collapsible=false ",
                "collapse_label=- tiers=[]\n",
            )
        );
    }

    #[test]
    fn inspects_solved_layout_tree() {
        let children = [
            LayoutNode::Slot(SlotNode {
                id: "sidebar",
                sizing: Sizing::fractional(0.25, 80.0),
                priority: 0.4,
                collapsible: true,
                display_tiers: TIERS,
                collapse_label: Some("Sidebar"),
            }),
            LayoutNode::Slot(SlotNode {
                id: "content",
                sizing: Sizing::flex(120.0),
                priority: 1.0,
                collapsible: false,
                display_tiers: &[],
                collapse_label: None,
            }),
        ];
        let root = sample_tree(&children);
        let solved = solve(&root, 300.0, 160.0, &LayoutPreferences::default());

        let inspection = inspect_solved(&solved);

        assert_eq!(inspection.nodes().len(), 3);
        assert_eq!(inspection.root.path, "root");
        assert_eq!(inspection.root.resolved_axis, Some(Axis::Horizontal));
        assert_eq!(inspection.root.children[0].path, "root/sidebar");
        assert_eq!(
            inspection.root.children[0].active_tier.as_deref(),
            Some("Mini")
        );
    }

    #[test]
    fn solved_layout_text_is_stable() {
        let children = [
            LayoutNode::Slot(SlotNode {
                id: "sidebar",
                sizing: Sizing::fractional(0.25, 80.0),
                priority: 0.4,
                collapsible: true,
                display_tiers: TIERS,
                collapse_label: Some("Sidebar"),
            }),
            LayoutNode::Slot(SlotNode {
                id: "content",
                sizing: Sizing::flex(120.0),
                priority: 1.0,
                collapsible: false,
                display_tiers: &[],
                collapse_label: None,
            }),
        ];
        let root = sample_tree(&children);
        let solved = solve(&root, 300.0, 160.0, &LayoutPreferences::default());

        let inspection = inspect_solved(&solved);

        assert_eq!(
            inspection.to_text(),
            concat!(
                "solved inspection:\n",
                "- root size=300x160 visible axis=horizontal tier=- collapse_label=- children=2\n",
                "  - root/sidebar size=80x160 visible axis=- tier=Mini ",
                "collapse_label=Sidebar children=0\n",
                "  - root/content size=216x160 visible axis=- tier=- ",
                "collapse_label=- children=0\n",
            )
        );
    }
}
