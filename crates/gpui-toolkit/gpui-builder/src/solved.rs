//! Solver output types.
//!
//! The solver produces a `SolvedNode` tree that mirrors the input `LayoutNode`
//! tree, with concrete pixel sizes and visibility states resolved.

use std::fmt;

use crate::types::{Axis, LayoutNode, Sizing};

/// A resolved node in the layout tree.
#[derive(Debug, Clone)]
pub struct SolvedNode {
    /// Matches the `id` from the source `LayoutNode`.
    pub id: String,
    /// Resolved width in pixels.
    pub width: f32,
    /// Resolved height in pixels.
    pub height: f32,
    /// Whether this node is visible (false = collapsed or hidden).
    pub visible: bool,
    /// Which display tier is active (for slots with `display_tiers`).
    /// `None` if no tier matches or node has no tiers.
    pub active_tier: Option<String>,
    /// Tab label if this slot was collapsed.
    pub collapse_label: Option<String>,
    /// The resolved axis for this container (`None` for slots).
    pub resolved_axis: Option<Axis>,
    /// Resolved children (empty for slots, populated for containers).
    pub children: Vec<SolvedNode>,
}

/// Textual diagnostics for a solved layout tree.
///
/// The report is intended for examples, debug panes, logs, and tests. It keeps
/// the tree output stable and exposes warnings as structured values so callers
/// can surface them however they like.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutDebugReport {
    tree: String,
    warnings: Vec<LayoutDebugWarning>,
}

impl LayoutDebugReport {
    /// Returns the stable, line-oriented solved tree.
    pub fn tree(&self) -> &str {
        &self.tree
    }

    /// Returns warnings found while inspecting the solved tree.
    pub fn warnings(&self) -> &[LayoutDebugWarning] {
        &self.warnings
    }

    /// Returns true when no diagnostic warnings were found.
    pub fn is_clean(&self) -> bool {
        self.warnings.is_empty()
    }

    /// Returns true when at least one diagnostic warning was found.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

impl fmt::Display for LayoutDebugReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.tree)?;
        if self.warnings.is_empty() {
            return Ok(());
        }

        writeln!(f, "warnings:")?;
        for warning in &self.warnings {
            writeln!(f, "- {warning}")?;
        }
        Ok(())
    }
}

/// A single warning emitted by a [`LayoutDebugReport`].
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutDebugWarning {
    /// Node where the suspicious outcome was detected.
    pub node_id: String,
    /// Warning category and related measurements.
    pub kind: LayoutDebugWarningKind,
}

impl fmt::Display for LayoutDebugWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            LayoutDebugWarningKind::InvalidSize { width, height } => write!(
                f,
                "{} has invalid size {}x{}",
                self.node_id,
                format_number(*width),
                format_number(*height)
            ),
            LayoutDebugWarningKind::InvisibleWithoutCollapseLabel => {
                write!(f, "{} is invisible without a collapse label", self.node_id)
            }
            LayoutDebugWarningKind::MainAxisOverflow {
                axis,
                used,
                available,
            } => write!(
                f,
                "{} children use {}px on the {} axis, exceeding {}px",
                self.node_id,
                format_number(*used),
                axis_name(*axis),
                format_number(*available)
            ),
            LayoutDebugWarningKind::CrossAxisOverflow {
                axis,
                child_id,
                used,
                available,
            } => write!(
                f,
                "{} child {} uses {}px on the {} axis, exceeding {}px",
                self.node_id,
                child_id,
                format_number(*used),
                axis_name(*axis),
                format_number(*available)
            ),
        }
    }
}

/// Warning categories emitted by [`LayoutDebugReport`].
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutDebugWarningKind {
    /// A node has a negative, NaN, or infinite size.
    InvalidSize { width: f32, height: f32 },
    /// A node is hidden but has no collapse label explaining how it is restored.
    InvisibleWithoutCollapseLabel,
    /// Visible children exceed their parent's available main-axis size.
    MainAxisOverflow {
        axis: Axis,
        used: f32,
        available: f32,
    },
    /// A visible child exceeds its parent's cross-axis size.
    CrossAxisOverflow {
        axis: Axis,
        child_id: String,
        used: f32,
        available: f32,
    },
}

impl SolvedNode {
    /// Find a solved node by id (depth-first search).
    pub fn find(&self, id: &str) -> Option<&SolvedNode> {
        if self.id == id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find(id) {
                return Some(found);
            }
        }
        None
    }

    /// Returns the size along the given axis.
    pub fn size_along(&self, axis: Axis) -> f32 {
        match axis {
            Axis::Horizontal => self.width,
            Axis::Vertical => self.height,
        }
    }

    /// Collect all collapsed nodes with their labels.
    pub fn collapsed_tabs(&self) -> Vec<(&str, &str)> {
        let mut tabs = Vec::new();
        self.collect_collapsed(&mut tabs);
        tabs
    }

    fn collect_collapsed<'a>(&'a self, tabs: &mut Vec<(&'a str, &'a str)>) {
        if !self.visible
            && let Some(ref label) = self.collapse_label
        {
            tabs.push((&self.id, label));
        }
        for child in &self.children {
            child.collect_collapsed(tabs);
        }
    }

    /// Build a stable textual report for this solved tree.
    ///
    /// This solved-only variant includes concrete sizes, visibility, active
    /// display tier, resolved container axis, and warnings for suspicious output.
    /// Use [`Self::debug_report_with_source`] when the source `LayoutNode` tree
    /// is available and you also want declared sizing metadata in each line.
    pub fn debug_report(&self) -> LayoutDebugReport {
        build_debug_report(self, None)
    }

    /// Build a stable textual report enriched with source layout metadata.
    ///
    /// When `source` mirrors the solved tree, each line includes the original
    /// sizing mode, collapsibility, and priority. If a solved node is missing
    /// from the source tree, the report still renders the solved node.
    pub fn debug_report_with_source(&self, source: &LayoutNode<'_>) -> LayoutDebugReport {
        build_debug_report(self, Some(source))
    }
}

const WARNING_EPSILON: f32 = 0.5;

fn build_debug_report(root: &SolvedNode, source: Option<&LayoutNode<'_>>) -> LayoutDebugReport {
    let mut lines = Vec::new();
    let mut warnings = Vec::new();
    append_debug_node(
        root,
        source.filter(|s| s.id() == root.id),
        0,
        &mut lines,
        &mut warnings,
    );
    LayoutDebugReport {
        tree: lines.join("\n"),
        warnings,
    }
}

fn append_debug_node(
    node: &SolvedNode,
    source: Option<&LayoutNode<'_>>,
    depth: usize,
    lines: &mut Vec<String>,
    warnings: &mut Vec<LayoutDebugWarning>,
) {
    let indent = "  ".repeat(depth);
    let mut line = format!(
        "{indent}{} size={}x{} {}",
        node.id,
        format_number(node.width),
        format_number(node.height),
        visibility_label(node),
    );

    if let Some(axis) = node.resolved_axis {
        line.push_str(" axis=");
        line.push_str(axis_name(axis));
    }

    if let Some(tier) = node.active_tier.as_deref() {
        line.push_str(" tier=");
        line.push_str(tier);
    }

    if let Some(label) = node.collapse_label.as_deref()
        && !node.visible
    {
        line.push_str(" label=");
        line.push_str(&format!("{label:?}"));
    }

    if let Some(source) = source {
        line.push_str(" sizing=");
        line.push_str(&format_sizing(source.sizing()));

        if source.collapsible() {
            line.push_str(" collapsible priority=");
            line.push_str(&format_number(source.priority()));
        }
    }

    lines.push(line);
    collect_node_warnings(node, warnings);

    for child in &node.children {
        let source_child = source_child(source, &child.id);
        append_debug_node(child, source_child, depth + 1, lines, warnings);
    }
}

fn collect_node_warnings(node: &SolvedNode, warnings: &mut Vec<LayoutDebugWarning>) {
    if !node.width.is_finite()
        || !node.height.is_finite()
        || node.width < -WARNING_EPSILON
        || node.height < -WARNING_EPSILON
    {
        warnings.push(LayoutDebugWarning {
            node_id: node.id.clone(),
            kind: LayoutDebugWarningKind::InvalidSize {
                width: node.width,
                height: node.height,
            },
        });
    }

    if !node.visible && node.collapse_label.is_none() {
        warnings.push(LayoutDebugWarning {
            node_id: node.id.clone(),
            kind: LayoutDebugWarningKind::InvisibleWithoutCollapseLabel,
        });
    }

    let Some(axis) = node.resolved_axis else {
        return;
    };
    if !node.visible {
        return;
    }

    let available = node.size_along(axis);
    let used: f32 = node
        .children
        .iter()
        .filter(|child| child.visible)
        .map(|child| child.size_along(axis))
        .sum();

    if available.is_finite() && used.is_finite() && used > available + WARNING_EPSILON {
        warnings.push(LayoutDebugWarning {
            node_id: node.id.clone(),
            kind: LayoutDebugWarningKind::MainAxisOverflow {
                axis,
                used,
                available,
            },
        });
    }

    let cross = axis.cross();
    let available_cross = node.size_along(cross);
    if !available_cross.is_finite() {
        return;
    }

    for child in node.children.iter().filter(|child| child.visible) {
        let child_cross = child.size_along(cross);
        if child_cross.is_finite() && child_cross > available_cross + WARNING_EPSILON {
            warnings.push(LayoutDebugWarning {
                node_id: node.id.clone(),
                kind: LayoutDebugWarningKind::CrossAxisOverflow {
                    axis: cross,
                    child_id: child.id.clone(),
                    used: child_cross,
                    available: available_cross,
                },
            });
        }
    }
}

fn source_child<'a>(
    source: Option<&'a LayoutNode<'a>>,
    child_id: &str,
) -> Option<&'a LayoutNode<'a>> {
    match source? {
        LayoutNode::Container(container) => container
            .children
            .iter()
            .find(|child| child.id() == child_id),
        LayoutNode::Slot(_) => None,
    }
}

fn visibility_label(node: &SolvedNode) -> &'static str {
    if node.visible { "visible" } else { "collapsed" }
}

fn axis_name(axis: Axis) -> &'static str {
    match axis {
        Axis::Horizontal => "horizontal",
        Axis::Vertical => "vertical",
    }
}

fn format_sizing(sizing: Sizing<'_>) -> String {
    match sizing {
        Sizing::Fixed(size) => format!("Fixed({})", format_number(size)),
        Sizing::Fractional { initial, min, max } => format!(
            "Fractional(initial={},min={},max={})",
            format_number(initial),
            format_number(min),
            if max == f32::MAX {
                "unbounded".to_string()
            } else {
                format_number(max)
            }
        ),
        Sizing::Flex { min, weight } => {
            format!(
                "Flex(min={},weight={})",
                format_number(min),
                format_number(weight)
            )
        }
        Sizing::Text {
            text,
            line_height,
            min,
            ..
        } => format!(
            "Text(chars={},line_height={},min={})",
            text.chars().count(),
            format_number(line_height),
            format_number(min)
        ),
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
    use crate::types::{ContainerNode, SlotNode};

    fn solved_slot(id: &str, width: f32, height: f32) -> SolvedNode {
        SolvedNode {
            id: id.to_string(),
            width,
            height,
            visible: true,
            active_tier: None,
            collapse_label: None,
            resolved_axis: None,
            children: Vec::new(),
        }
    }

    #[test]
    fn debug_report_includes_source_metadata_and_collapsed_labels() {
        let source_children = [
            LayoutNode::Slot(SlotNode {
                id: "header",
                sizing: Sizing::Fixed(40.0),
                priority: 1.0,
                collapsible: false,
                display_tiers: &[],
                collapse_label: None,
            }),
            LayoutNode::Slot(SlotNode {
                id: "inspector",
                sizing: Sizing::fractional(0.25, 80.0),
                priority: 0.4,
                collapsible: true,
                display_tiers: &[],
                collapse_label: Some("Inspector"),
            }),
        ];
        let source = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Vertical,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &source_children,
            divider_size: 0.0,
        });
        let solved = SolvedNode {
            id: "root".to_string(),
            width: 320.0,
            height: 240.0,
            visible: true,
            active_tier: None,
            collapse_label: None,
            resolved_axis: Some(Axis::Vertical),
            children: vec![
                solved_slot("header", 320.0, 40.0),
                SolvedNode {
                    id: "inspector".to_string(),
                    width: 0.0,
                    height: 0.0,
                    visible: false,
                    active_tier: None,
                    collapse_label: Some("Inspector".to_string()),
                    resolved_axis: None,
                    children: Vec::new(),
                },
            ],
        };

        let report = solved.debug_report_with_source(&source);

        assert!(report.is_clean());
        assert_eq!(
            report.tree(),
            concat!(
                "root size=320x240 visible axis=vertical sizing=Flex(min=0,weight=1)\n",
                "  header size=320x40 visible sizing=Fixed(40)\n",
                "  inspector size=0x0 collapsed label=\"Inspector\" ",
                "sizing=Fractional(initial=0.25,min=80,max=unbounded) collapsible priority=0.4",
            )
        );
    }

    #[test]
    fn debug_report_warns_for_invalid_hidden_and_overflowing_nodes() {
        let solved = SolvedNode {
            id: "root".to_string(),
            width: 100.0,
            height: 40.0,
            visible: true,
            active_tier: None,
            collapse_label: None,
            resolved_axis: Some(Axis::Horizontal),
            children: vec![
                solved_slot("wide", 75.0, 45.0),
                solved_slot("wider", 50.0, 20.0),
                SolvedNode {
                    id: "ghost".to_string(),
                    width: f32::NAN,
                    height: 0.0,
                    visible: false,
                    active_tier: None,
                    collapse_label: None,
                    resolved_axis: None,
                    children: Vec::new(),
                },
            ],
        };

        let report = solved.debug_report();

        assert!(report.has_warnings());
        assert_eq!(report.warnings().len(), 4);
        assert_eq!(
            report.warnings()[0],
            LayoutDebugWarning {
                node_id: "root".to_string(),
                kind: LayoutDebugWarningKind::MainAxisOverflow {
                    axis: Axis::Horizontal,
                    used: 125.0,
                    available: 100.0,
                },
            }
        );
        assert_eq!(
            report.warnings()[1],
            LayoutDebugWarning {
                node_id: "root".to_string(),
                kind: LayoutDebugWarningKind::CrossAxisOverflow {
                    axis: Axis::Vertical,
                    child_id: "wide".to_string(),
                    used: 45.0,
                    available: 40.0,
                },
            }
        );
        match &report.warnings()[2].kind {
            LayoutDebugWarningKind::InvalidSize { width, height } => {
                assert_eq!(report.warnings()[2].node_id, "ghost");
                assert!(width.is_nan());
                assert_eq!(*height, 0.0);
            }
            other => panic!("expected invalid-size warning, got {other:?}"),
        }
        assert_eq!(
            report.warnings()[3],
            LayoutDebugWarning {
                node_id: "ghost".to_string(),
                kind: LayoutDebugWarningKind::InvisibleWithoutCollapseLabel,
            }
        );
        assert!(
            report
                .to_string()
                .contains("warnings:\n- root children use 125px")
        );
    }
}
