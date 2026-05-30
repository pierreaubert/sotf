//! Layout declaration validation.
//!
//! The solver is intentionally permissive and deterministic. This module adds
//! a separate lint pass for examples, tests, and CI so suspicious declarations
//! can be caught before a layout is solved.

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::{ContainerNode, DisplayTier, LayoutNode, Sizing, SlotNode};

/// Severity for a layout validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutIssueSeverity {
    /// A declaration is invalid or ambiguous enough that solving it is risky.
    Error,
    /// A declaration is accepted by the solver but likely hurts UX, tooling, or accessibility.
    Warning,
}

impl fmt::Display for LayoutIssueSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "ERROR"),
            Self::Warning => write!(f, "WARNING"),
        }
    }
}

/// Machine-readable category for a validation issue.
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutIssueKind {
    EmptyId,
    DuplicateId { first_path: String },
    InvalidSizing,
    InvalidAutoAxis,
    InvalidDividerSize,
    InvalidPriority,
    PriorityOutOfRange,
    MissingCollapseLabel,
    EmptyCollapseLabel,
    InvalidDisplayTier,
    DuplicateDisplayTierName { name: String },
    DuplicateDisplayTierThreshold { min_size: f32 },
    DisplayTiersNotDescending,
    EmptyContainer,
}

/// One validation issue found in a layout tree.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutIssue {
    /// Error or warning severity.
    pub severity: LayoutIssueSeverity,
    /// Node id where the issue was found.
    pub node_id: String,
    /// Slash-separated tree path where the issue was found.
    pub path: String,
    /// Machine-readable issue kind.
    pub kind: LayoutIssueKind,
    /// Stable human-readable message.
    pub message: String,
}

/// Validation result for a layout tree.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutValidationReport {
    issues: Vec<LayoutIssue>,
}

impl LayoutValidationReport {
    /// Return all issues in deterministic depth-first order.
    pub fn issues(&self) -> &[LayoutIssue] {
        &self.issues
    }

    /// Return true when there are no errors or warnings.
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }

    /// Return true when at least one error was found.
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == LayoutIssueSeverity::Error)
    }

    /// Return true when at least one warning was found.
    pub fn has_warnings(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == LayoutIssueSeverity::Warning)
    }

    /// Count validation errors.
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == LayoutIssueSeverity::Error)
            .count()
    }

    /// Count validation warnings.
    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == LayoutIssueSeverity::Warning)
            .count()
    }

    /// Render a stable line-oriented report.
    pub fn to_text(&self) -> String {
        if self.issues.is_empty() {
            return "layout validation: clean\n".to_string();
        }

        let mut output = format!(
            "layout validation: {} error(s), {} warning(s)\n",
            self.error_count(),
            self.warning_count()
        );
        for issue in &self.issues {
            output.push_str(&format!(
                "- {} {}: {}\n",
                issue.severity, issue.path, issue.message
            ));
        }
        output
    }
}

impl fmt::Display for LayoutValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_text())
    }
}

/// Validate a layout declaration tree without solving it.
pub fn validate_layout(root: &LayoutNode<'_>) -> LayoutValidationReport {
    let mut state = ValidationState::default();
    validate_node(root, None, &mut state);
    LayoutValidationReport {
        issues: state.issues,
    }
}

#[derive(Default)]
struct ValidationState {
    seen_ids: HashMap<String, String>,
    issues: Vec<LayoutIssue>,
}

fn validate_node(node: &LayoutNode<'_>, parent_path: Option<&str>, state: &mut ValidationState) {
    let id = node.id();
    let path = node_path(parent_path, id);

    validate_common_node(node, &path, state);

    match node {
        LayoutNode::Slot(slot) => validate_slot(slot, &path, state),
        LayoutNode::Container(container) => {
            validate_container(container, &path, state);
            for child in container.children {
                validate_node(child, Some(&path), state);
            }
        }
    }
}

fn validate_common_node(node: &LayoutNode<'_>, path: &str, state: &mut ValidationState) {
    let id = node.id();

    if id.trim().is_empty() {
        push_issue(
            state,
            LayoutIssueSeverity::Error,
            id,
            path,
            LayoutIssueKind::EmptyId,
            "id must not be empty",
        );
    } else if let Some(first_path) = state.seen_ids.get(id).cloned() {
        push_issue(
            state,
            LayoutIssueSeverity::Error,
            id,
            path,
            LayoutIssueKind::DuplicateId {
                first_path: first_path.clone(),
            },
            format!("duplicate id {id:?} (first seen at {first_path})"),
        );
    } else {
        state.seen_ids.insert(id.to_string(), path.to_string());
    }

    validate_sizing(node.id(), path, node.sizing(), state);
}

fn validate_slot(slot: &SlotNode<'_>, path: &str, state: &mut ValidationState) {
    if !slot.priority.is_finite() {
        push_issue(
            state,
            LayoutIssueSeverity::Error,
            slot.id,
            path,
            LayoutIssueKind::InvalidPriority,
            format!(
                "priority must be finite, got {}",
                format_number(slot.priority)
            ),
        );
    } else if !(0.0..=1.0).contains(&slot.priority) {
        push_issue(
            state,
            LayoutIssueSeverity::Warning,
            slot.id,
            path,
            LayoutIssueKind::PriorityOutOfRange,
            format!(
                "priority should be in 0.0..=1.0, got {}",
                format_number(slot.priority)
            ),
        );
    }

    if slot.collapsible && slot.collapse_label.is_none() {
        push_issue(
            state,
            LayoutIssueSeverity::Warning,
            slot.id,
            path,
            LayoutIssueKind::MissingCollapseLabel,
            "collapsible slot should set collapse_label for restore UI/accessibility",
        );
    }

    if let Some(label) = slot.collapse_label
        && label.trim().is_empty()
    {
        push_issue(
            state,
            LayoutIssueSeverity::Warning,
            slot.id,
            path,
            LayoutIssueKind::EmptyCollapseLabel,
            "collapse_label should not be empty",
        );
    }

    validate_display_tiers(slot.id, path, slot.display_tiers, state);
}

fn validate_container(container: &ContainerNode<'_>, path: &str, state: &mut ValidationState) {
    if let Some(threshold) = container.auto_axis
        && (!threshold.is_finite() || threshold <= 0.0)
    {
        push_issue(
            state,
            LayoutIssueSeverity::Error,
            container.id,
            path,
            LayoutIssueKind::InvalidAutoAxis,
            format!(
                "auto_axis threshold must be finite and > 0, got {}",
                format_number(threshold)
            ),
        );
    }

    if !container.divider_size.is_finite() || container.divider_size < 0.0 {
        push_issue(
            state,
            LayoutIssueSeverity::Error,
            container.id,
            path,
            LayoutIssueKind::InvalidDividerSize,
            format!(
                "divider_size must be finite and >= 0, got {}",
                format_number(container.divider_size)
            ),
        );
    }

    if container.children.is_empty() {
        push_issue(
            state,
            LayoutIssueSeverity::Warning,
            container.id,
            path,
            LayoutIssueKind::EmptyContainer,
            "container has no children",
        );
    }
}

fn validate_sizing(node_id: &str, path: &str, sizing: Sizing<'_>, state: &mut ValidationState) {
    match sizing {
        Sizing::Fixed(size) => {
            if !is_non_negative(size) {
                push_invalid_sizing(
                    state,
                    node_id,
                    path,
                    format!(
                        "Fixed size must be finite and >= 0, got {}",
                        format_number(size)
                    ),
                );
            }
        }
        Sizing::Fractional { initial, min, max } => {
            if !initial.is_finite() || !(0.0..=1.0).contains(&initial) {
                push_invalid_sizing(
                    state,
                    node_id,
                    path,
                    format!(
                        "Fractional initial must be finite and in 0.0..=1.0, got {}",
                        format_number(initial)
                    ),
                );
            }
            if !is_non_negative(min) {
                push_invalid_sizing(
                    state,
                    node_id,
                    path,
                    format!(
                        "Fractional min must be finite and >= 0, got {}",
                        format_number(min)
                    ),
                );
            }
            if !is_non_negative(max) {
                push_invalid_sizing(
                    state,
                    node_id,
                    path,
                    format!(
                        "Fractional max must be finite and >= 0, got {}",
                        format_number(max)
                    ),
                );
            } else if max < min {
                push_invalid_sizing(
                    state,
                    node_id,
                    path,
                    format!(
                        "Fractional max must be >= min (min={}, max={})",
                        format_number(min),
                        format_number(max)
                    ),
                );
            }
        }
        Sizing::Flex { min, weight } => {
            if !is_non_negative(min) {
                push_invalid_sizing(
                    state,
                    node_id,
                    path,
                    format!(
                        "Flex min must be finite and >= 0, got {}",
                        format_number(min)
                    ),
                );
            }
            if !weight.is_finite() || weight <= 0.0 {
                push_invalid_sizing(
                    state,
                    node_id,
                    path,
                    format!(
                        "Flex weight must be finite and > 0, got {}",
                        format_number(weight)
                    ),
                );
            }
        }
        Sizing::Text {
            line_height, min, ..
        } => {
            if !line_height.is_finite() || line_height <= 0.0 {
                push_invalid_sizing(
                    state,
                    node_id,
                    path,
                    format!(
                        "Text line_height must be finite and > 0, got {}",
                        format_number(line_height)
                    ),
                );
            }
            if !is_non_negative(min) {
                push_invalid_sizing(
                    state,
                    node_id,
                    path,
                    format!(
                        "Text min must be finite and >= 0, got {}",
                        format_number(min)
                    ),
                );
            }
        }
    }
}

fn validate_display_tiers(
    node_id: &str,
    path: &str,
    tiers: &[DisplayTier<'_>],
    state: &mut ValidationState,
) {
    let mut names = HashSet::new();
    let mut thresholds = HashSet::new();
    let mut previous_min_size: Option<f32> = None;
    let mut descending_warning_emitted = false;

    for tier in tiers {
        if tier.name.trim().is_empty() {
            push_issue(
                state,
                LayoutIssueSeverity::Warning,
                node_id,
                path,
                LayoutIssueKind::InvalidDisplayTier,
                "display tier name should not be empty",
            );
        } else if !names.insert(tier.name) {
            push_issue(
                state,
                LayoutIssueSeverity::Warning,
                node_id,
                path,
                LayoutIssueKind::DuplicateDisplayTierName {
                    name: tier.name.to_string(),
                },
                format!("duplicate display tier name {:?}", tier.name),
            );
        }

        if !is_non_negative(tier.min_size) {
            push_issue(
                state,
                LayoutIssueSeverity::Error,
                node_id,
                path,
                LayoutIssueKind::InvalidDisplayTier,
                format!(
                    "display tier {:?} min_size must be finite and >= 0, got {}",
                    tier.name,
                    format_number(tier.min_size)
                ),
            );
        } else {
            let threshold_key = tier.min_size.to_bits();
            if !thresholds.insert(threshold_key) {
                push_issue(
                    state,
                    LayoutIssueSeverity::Warning,
                    node_id,
                    path,
                    LayoutIssueKind::DuplicateDisplayTierThreshold {
                        min_size: tier.min_size,
                    },
                    format!(
                        "duplicate display tier min_size {}",
                        format_number(tier.min_size)
                    ),
                );
            }

            if let Some(previous) = previous_min_size
                && tier.min_size > previous
                && !descending_warning_emitted
            {
                descending_warning_emitted = true;
                push_issue(
                    state,
                    LayoutIssueSeverity::Warning,
                    node_id,
                    path,
                    LayoutIssueKind::DisplayTiersNotDescending,
                    "display tiers should be ordered from largest min_size to smallest",
                );
            }
            previous_min_size = Some(tier.min_size);
        }
    }
}

fn push_invalid_sizing(state: &mut ValidationState, node_id: &str, path: &str, message: String) {
    push_issue(
        state,
        LayoutIssueSeverity::Error,
        node_id,
        path,
        LayoutIssueKind::InvalidSizing,
        message,
    );
}

fn push_issue(
    state: &mut ValidationState,
    severity: LayoutIssueSeverity,
    node_id: &str,
    path: &str,
    kind: LayoutIssueKind,
    message: impl Into<String>,
) {
    state.issues.push(LayoutIssue {
        severity,
        node_id: node_id.to_string(),
        path: path.to_string(),
        kind,
        message: message.into(),
    });
}

fn is_non_negative(value: f32) -> bool {
    value.is_finite() && value >= 0.0
}

fn node_path(parent_path: Option<&str>, id: &str) -> String {
    let segment = if id.is_empty() { "<empty>" } else { id };
    match parent_path {
        Some(parent_path) => format!("{parent_path}/{segment}"),
        None => segment.to_string(),
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
    use crate::{Axis, LayoutNode};

    #[test]
    fn clean_layout_has_no_issues() {
        static TIERS: &[DisplayTier<'_>] = &[
            DisplayTier {
                name: "Full",
                min_size: 200.0,
            },
            DisplayTier {
                name: "Mini",
                min_size: 80.0,
            },
        ];
        let children = [
            LayoutNode::Slot(SlotNode {
                id: "main",
                sizing: Sizing::flex(200.0),
                priority: 1.0,
                collapsible: false,
                display_tiers: TIERS,
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
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: Some(1.0),
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: 6.0,
        });

        let report = validate_layout(&root);

        assert!(report.is_clean(), "{report}");
        assert_eq!(report.to_text(), "layout validation: clean\n");
    }

    #[test]
    fn validation_report_text_is_stable() {
        let children = [
            LayoutNode::Slot(SlotNode {
                id: "panel",
                sizing: Sizing::Fixed(-1.0),
                priority: 1.5,
                collapsible: true,
                display_tiers: &[],
                collapse_label: None,
            }),
            LayoutNode::Slot(SlotNode {
                id: "panel",
                sizing: Sizing::flex(20.0),
                priority: 1.0,
                collapsible: false,
                display_tiers: &[],
                collapse_label: None,
            }),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: Some(0.0),
            sizing: Sizing::flex(0.0),
            children: &children,
            divider_size: -2.0,
        });

        let report = validate_layout(&root);

        assert!(report.has_errors());
        assert!(report.has_warnings());
        assert_eq!(report.error_count(), 4);
        assert_eq!(report.warning_count(), 2);
        assert_eq!(
            report.to_text(),
            concat!(
                "layout validation: 4 error(s), 2 warning(s)\n",
                "- ERROR root: auto_axis threshold must be finite and > 0, got 0\n",
                "- ERROR root: divider_size must be finite and >= 0, got -2\n",
                "- ERROR root/panel: Fixed size must be finite and >= 0, got -1\n",
                "- WARNING root/panel: priority should be in 0.0..=1.0, got 1.5\n",
                "- WARNING root/panel: collapsible slot should set collapse_label ",
                "for restore UI/accessibility\n",
                "- ERROR root/panel: duplicate id \"panel\" (first seen at root/panel)\n",
            )
        );
    }

    #[test]
    fn warns_for_tier_and_container_quality_issues() {
        static TIERS: &[DisplayTier<'_>] = &[
            DisplayTier {
                name: "Mini",
                min_size: 80.0,
            },
            DisplayTier {
                name: "Mini",
                min_size: 200.0,
            },
            DisplayTier {
                name: "",
                min_size: 200.0,
            },
        ];
        let children = [LayoutNode::Slot(SlotNode {
            id: "slot",
            sizing: Sizing::fractional(0.5, 10.0),
            priority: 1.0,
            collapsible: true,
            display_tiers: TIERS,
            collapse_label: Some(" "),
        })];
        let empty_children = [];
        let root_children = [
            children[0],
            LayoutNode::Container(ContainerNode {
                id: "empty",
                axis: Axis::Vertical,
                auto_axis: None,
                sizing: Sizing::flex(0.0),
                children: &empty_children,
                divider_size: 0.0,
            }),
        ];
        let root = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Vertical,
            auto_axis: None,
            sizing: Sizing::flex(0.0),
            children: &root_children,
            divider_size: 0.0,
        });

        let report = validate_layout(&root);

        assert_eq!(report.error_count(), 0);
        assert_eq!(report.warning_count(), 6);
        assert!(
            report
                .issues()
                .iter()
                .any(|issue| issue.kind == LayoutIssueKind::EmptyCollapseLabel)
        );
        assert!(
            report
                .issues()
                .iter()
                .any(|issue| issue.kind == LayoutIssueKind::DisplayTiersNotDescending)
        );
        assert!(
            report
                .issues()
                .iter()
                .any(|issue| issue.kind == LayoutIssueKind::EmptyContainer)
        );
    }
}
