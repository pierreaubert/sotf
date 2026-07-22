use crate::plugin_layout::{ControlGroup, ControlType, GroupOverflow};
use gpui_builder::{
    Axis, CollapsedSlot, ContainerNode, LayoutNode, LayoutPreferences, Sizing, SlotNode,
    SolvedTree, solve_tree_into,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;

const DEFAULT_GROUP_WIDTH: f32 = 150.0;
const WIDE_GROUP_WIDTH: f32 = 316.0;
const GROUP_GAP: f32 = 16.0;

thread_local! {
    static GROUP_DECLARATIONS: RefCell<HashMap<Vec<usize>, &'static [LayoutNode<'static>]>> =
        RefCell::new(HashMap::new());
    static GROUP_SOLVED_TREES: RefCell<HashMap<usize, SolvedTree<'static>>> =
        RefCell::new(HashMap::new());
}

/// Invalid metadata in a generated plugin control-group declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupLayoutError {
    message: String,
}

impl GroupLayoutError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for GroupLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for GroupLayoutError {}

/// Visibility result for one atomic control group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SolvedControlGroup<'a> {
    id: &'a str,
    label: &'a str,
    visible: bool,
}

impl<'a> SolvedControlGroup<'a> {
    pub const fn id(self) -> &'a str {
        self.id
    }

    pub const fn visible(self) -> bool {
        self.visible
    }
}

/// Framework-neutral result of solving a generated plugin's group layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolvedControlGroups<'a> {
    groups: Vec<SolvedControlGroup<'a>>,
}

impl<'a> SolvedControlGroups<'a> {
    pub fn find(&self, id: &str) -> Option<SolvedControlGroup<'a>> {
        self.groups.iter().copied().find(|group| group.id == id)
    }

    pub fn collapsed_slots(&self) -> impl Iterator<Item = CollapsedSlot<'a>> + '_ {
        self.groups
            .iter()
            .filter(|group| !group.visible)
            .map(|group| CollapsedSlot {
                id: group.id,
                label: group.label,
            })
    }
}

/// Validate stable IDs and responsive sizing metadata for active groups.
pub fn validate_control_groups(groups: &[&ControlGroup]) -> Result<(), GroupLayoutError> {
    let mut ids = HashSet::with_capacity(groups.len());
    for (index, group) in groups.iter().enumerate() {
        if group.id.is_empty() {
            return Err(GroupLayoutError::new("control-group id must not be empty"));
        }
        if !ids.insert(group.id) {
            return Err(GroupLayoutError::new(format!(
                "duplicate control-group id '{}'",
                group.id
            )));
        }
        let priority = group.layout.collapse_priority;
        if !priority.is_finite() || !(0.0..=1.0).contains(&priority) {
            return Err(GroupLayoutError::new(format!(
                "control-group '{}' priority must be finite and within 0..=1",
                group.id
            )));
        }
        if index > 0 && group.layout.overflow == GroupOverflow::Auto && group.title.is_empty() {
            return Err(GroupLayoutError::new(format!(
                "overflowable control-group '{}' must have a non-empty label",
                group.id
            )));
        }
        let (inferred_min, inferred_preferred) = inferred_group_widths(group);
        let min = group.layout.min_width.unwrap_or(inferred_min);
        let preferred = group.layout.preferred_width.unwrap_or(inferred_preferred);
        if !min.is_finite() || !preferred.is_finite() || min <= 0.0 || preferred < min {
            return Err(GroupLayoutError::new(format!(
                "control-group '{}' widths must be positive, finite, and ordered",
                group.id
            )));
        }
    }
    Ok(())
}

/// Infer a group's minimum and preferred widths from its visible control types.
pub fn inferred_group_widths(group: &ControlGroup) -> (f32, f32) {
    let visible_count = group
        .controls
        .iter()
        .filter(|control| !control.hidden)
        .count();
    if visible_count == 0 {
        return (DEFAULT_GROUP_WIDTH, DEFAULT_GROUP_WIDTH);
    }

    if group
        .controls
        .iter()
        .filter(|control| !control.hidden)
        .any(|control| matches!(control.control_type, ControlType::VerticalSlider))
    {
        let width = 72.0 * visible_count as f32;
        return (width, width);
    }

    let type_min = group
        .controls
        .iter()
        .filter(|control| !control.hidden)
        .fold(DEFAULT_GROUP_WIDTH, |width, control| {
            width.max(match control.control_type {
                ControlType::Knob => 130.0,
                ControlType::KnobLarge => 170.0,
                ControlType::VerticalSlider => 72.0,
                ControlType::Toggle | ControlType::Selector | ControlType::Label => 150.0,
                ControlType::ButtonSet { .. } | ControlType::FilePicker => 180.0,
                ControlType::BarMeter { .. } => 130.0,
            })
        });
    let preferred = if visible_count >= 4 {
        WIDE_GROUP_WIDTH.max(type_min)
    } else {
        type_min
    };
    (type_min, preferred)
}

fn cached_declaration(groups: &[&ControlGroup]) -> &'static [LayoutNode<'static>] {
    let key: Vec<_> = groups
        .iter()
        .map(|group| std::ptr::from_ref(*group) as usize)
        .collect();
    GROUP_DECLARATIONS.with(|declarations| {
        let mut declarations = declarations.borrow_mut();
        *declarations.entry(key).or_insert_with(|| {
            let total_preferred: f32 = groups
                .iter()
                .map(|group| {
                    group
                        .layout
                        .preferred_width
                        .unwrap_or_else(|| inferred_group_widths(group).1)
                })
                .sum::<f32>()
                + GROUP_GAP * groups.len().saturating_sub(1) as f32;

            let children: Vec<LayoutNode<'static>> = groups
                .iter()
                .enumerate()
                .map(|(index, group)| {
                    let (inferred_min, inferred_preferred) = inferred_group_widths(group);
                    let min = group.layout.min_width.unwrap_or(inferred_min);
                    let preferred = group.layout.preferred_width.unwrap_or(inferred_preferred);
                    let keep_visible =
                        index == 0 || group.layout.overflow == GroupOverflow::KeepVisible;
                    LayoutNode::Slot(SlotNode {
                        id: group.id,
                        sizing: Sizing::Fractional {
                            initial: preferred / total_preferred.max(preferred),
                            min,
                            max: preferred,
                        },
                        priority: group.layout.collapse_priority,
                        collapsible: !keep_visible,
                        display_tiers: &[],
                        collapse_label: (!keep_visible).then_some(group.title),
                    })
                })
                .collect();
            Box::leak(children.into_boxed_slice())
        })
    })
}

/// Solve atomic control-group visibility with the generic constraint solver.
///
/// Equal priorities overflow later declarations first. The first active group
/// is pinned so a narrow layout never degrades to only an overflow trigger.
pub fn solve_control_groups<'a>(
    groups: &[&'a ControlGroup],
    available_width: f32,
) -> Result<SolvedControlGroups<'a>, GroupLayoutError> {
    validate_control_groups(groups)?;

    let children = cached_declaration(groups);
    let root = ContainerNode::new(
        "plugin-main-groups",
        Axis::Horizontal,
        Sizing::flex(0.0),
        children,
    )
    .divider_size(GROUP_GAP)
    .into_node();

    let declaration_key = children.as_ptr() as usize;
    GROUP_SOLVED_TREES.with(|trees| {
        let mut trees = trees.borrow_mut();
        let solved = trees
            .entry(declaration_key)
            .or_insert_with(|| SolvedTree::with_capacity(children.len() + 1));
        solve_tree_into(
            &root,
            available_width.max(0.0),
            1.0,
            &LayoutPreferences::default(),
            solved,
        );
        Ok(SolvedControlGroups {
            groups: groups
                .iter()
                .map(|group| SolvedControlGroup {
                    id: group.id,
                    label: group.title,
                    visible: solved.find(group.id).is_some_and(|node| node.visible()),
                })
                .collect(),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_layout::{ControlSpec, GroupLayoutHints};

    static CONTROLS: [ControlSpec; 1] = [ControlSpec::knob(0)];
    static FIRST: ControlGroup = ControlGroup::new("first", "First", &CONTROLS);
    static SECOND: ControlGroup = ControlGroup::new("second", "Second", &CONTROLS);
    static THIRD: ControlGroup = ControlGroup::new("third", "Third", &CONTROLS);

    #[test]
    fn later_equal_priority_groups_overflow_first_and_primary_is_pinned() {
        let groups = [&FIRST, &SECOND, &THIRD];
        let solved = solve_control_groups(&groups, 140.0).unwrap();
        let collapsed: Vec<_> = solved.collapsed_slots().map(|slot| slot.id).collect();
        assert_eq!(collapsed, vec!["second", "third"]);
        assert!(solved.find("first").unwrap().visible());
    }

    #[test]
    fn declarations_are_cached_per_variant_and_resize_is_deterministic() {
        let full = [&FIRST, &SECOND, &THIRD];
        let alternate = [&FIRST, &THIRD];
        assert_eq!(
            cached_declaration(&full).as_ptr(),
            cached_declaration(&full).as_ptr()
        );
        assert_ne!(
            cached_declaration(&full).as_ptr(),
            cached_declaration(&alternate).as_ptr()
        );

        let narrow_before: Vec<_> = solve_control_groups(&full, 160.0)
            .unwrap()
            .collapsed_slots()
            .map(|slot| slot.id)
            .collect();
        assert!(
            solve_control_groups(&full, 800.0)
                .unwrap()
                .collapsed_slots()
                .next()
                .is_none()
        );
        let narrow_after: Vec<_> = solve_control_groups(&full, 160.0)
            .unwrap()
            .collapsed_slots()
            .map(|slot| slot.id)
            .collect();
        assert_eq!(narrow_before, narrow_after);
    }

    #[test]
    fn explicit_hints_override_inferred_widths() {
        static CUSTOM: ControlGroup = ControlGroup::new("custom", "Custom", &CONTROLS).with_layout(
            GroupLayoutHints::inferred()
                .widths(210.0, 260.0)
                .priority(0.8),
        );
        assert_eq!(inferred_group_widths(&CUSTOM), (150.0, 150.0));
        assert_eq!(CUSTOM.layout.min_width, Some(210.0));
        assert_eq!(CUSTOM.layout.preferred_width, Some(260.0));
    }

    #[test]
    fn width_hints_may_override_minimum_or_preferred_independently() {
        static MIN_ONLY: ControlGroup = ControlGroup::new("min-only", "Min", &CONTROLS)
            .with_layout(GroupLayoutHints {
                min_width: Some(120.0),
                ..GroupLayoutHints::inferred()
            });
        static PREFERRED_ONLY: ControlGroup =
            ControlGroup::new("preferred-only", "Preferred", &CONTROLS).with_layout(
                GroupLayoutHints {
                    preferred_width: Some(220.0),
                    ..GroupLayoutHints::inferred()
                },
            );

        assert!(validate_control_groups(&[&MIN_ONLY, &PREFERRED_ONLY]).is_ok());
    }

    #[test]
    fn validation_rejects_invalid_metadata() {
        static EMPTY_ID: ControlGroup = ControlGroup::new("", "Label", &CONTROLS);
        static EMPTY_LABEL: ControlGroup = ControlGroup::new("empty-label", "", &CONTROLS);
        static BAD_WIDTH: ControlGroup = ControlGroup::new("bad-width", "Bad", &CONTROLS)
            .with_layout(GroupLayoutHints::inferred().widths(200.0, 100.0));
        assert!(validate_control_groups(&[&EMPTY_ID]).is_err());
        assert!(validate_control_groups(&[&FIRST, &EMPTY_LABEL]).is_err());
        assert!(validate_control_groups(&[&BAD_WIDTH]).is_err());
        assert!(validate_control_groups(&[&FIRST, &FIRST]).is_err());
    }
}
