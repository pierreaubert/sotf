//! Declarative layout types.
//!
//! Platform-agnostic data types for describing layout trees. No rendering code,
//! no GPUI framework dependencies. Consumed by the solver to produce resolved geometry.

use gpui_pretext::TextMeasure;

// ============================================================================
// Axis & Direction
// ============================================================================

/// Primary axis along which children are arranged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// Children laid out left-to-right; main dimension is width.
    Horizontal,
    /// Children laid out top-to-bottom; main dimension is height.
    Vertical,
}

impl Axis {
    /// Returns the perpendicular axis.
    pub fn cross(self) -> Self {
        match self {
            Axis::Horizontal => Axis::Vertical,
            Axis::Vertical => Axis::Horizontal,
        }
    }
}

// ============================================================================
// Sizing Constraints
// ============================================================================

/// How a node claims space within its parent's main axis.
///
/// The `'a` lifetime is used only by `Sizing::Text`, which borrows the text
/// string and a [`TextMeasure`] implementation. All other variants are effectively
/// `'static` and can be used with any lifetime.
#[derive(Clone, Copy)]
pub enum Sizing<'a> {
    /// Fixed size in pixels. Always gets exactly this much space.
    /// Use for: headers, footers, toolbars, fixed-width sidebars.
    Fixed(f32),

    /// Fractional: claims a fraction of remaining space (after fixed allocations).
    /// `initial` is the default ratio (0.0..=1.0); user preferences can override it.
    /// `min` and `max` are hard pixel bounds.
    /// Use for: resizable side panels (library, rack).
    Fractional { initial: f32, min: f32, max: f32 },

    /// Flex: takes all remaining space after siblings are allocated.
    /// If multiple Flex siblings exist, they split remaining space by weight.
    /// `min` is the absolute minimum in pixels.
    /// Use for: main content areas (queue panel, plugin main column).
    Flex { min: f32, weight: f32 },

    /// Text-measured: size is computed by laying out `text` with gpui-pretext.
    ///
    /// - In a **vertical** container (main axis = height): allocates the text's
    ///   wrapped height given the container's full width as `max_width`.
    /// - In a **horizontal** container (main axis = width): allocates the width
    ///   of the longest text line (no wrapping).
    ///
    /// `min` is a pixel floor applied after measurement.
    Text {
        text: &'a str,
        measure: &'a dyn TextMeasure,
        line_height: f32,
        min: f32,
    },
}

impl<'a> std::fmt::Debug for Sizing<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Sizing::Fixed(v) => write!(f, "Fixed({v})"),
            Sizing::Fractional { initial, min, max } => {
                write!(
                    f,
                    "Fractional {{ initial: {initial}, min: {min}, max: {max} }}"
                )
            }
            Sizing::Flex { min, weight } => write!(f, "Flex {{ min: {min}, weight: {weight} }}"),
            Sizing::Text {
                text,
                line_height,
                min,
                ..
            } => {
                write!(
                    f,
                    "Text {{ text: {:?}, line_height: {line_height}, min: {min} }}",
                    text
                )
            }
        }
    }
}

impl<'a> PartialEq for Sizing<'a> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Sizing::Fixed(a), Sizing::Fixed(b)) => a == b,
            (
                Sizing::Fractional {
                    initial: i1,
                    min: mn1,
                    max: mx1,
                },
                Sizing::Fractional {
                    initial: i2,
                    min: mn2,
                    max: mx2,
                },
            ) => i1 == i2 && mn1 == mn2 && mx1 == mx2,
            (
                Sizing::Flex {
                    min: mn1,
                    weight: w1,
                },
                Sizing::Flex {
                    min: mn2,
                    weight: w2,
                },
            ) => mn1 == mn2 && w1 == w2,
            (
                Sizing::Text {
                    text: t1,
                    measure: m1,
                    line_height: lh1,
                    min: mn1,
                },
                Sizing::Text {
                    text: t2,
                    measure: m2,
                    line_height: lh2,
                    min: mn2,
                },
            ) => {
                t1 == t2
                    && lh1 == lh2
                    && mn1 == mn2
                    && std::ptr::eq(*m1 as *const dyn TextMeasure, *m2 as *const dyn TextMeasure)
            }
            _ => false,
        }
    }
}

impl<'a> Sizing<'a> {
    /// Shorthand for `Flex` with weight 1.0.
    pub const fn flex(min: f32) -> Self {
        Sizing::Flex { min, weight: 1.0 }
    }

    /// Shorthand for `Fractional` with no max.
    pub const fn fractional(initial: f32, min: f32) -> Self {
        Sizing::Fractional {
            initial,
            min,
            max: f32::MAX,
        }
    }

    /// Returns the minimum size this node needs along the main axis.
    ///
    /// For `Sizing::Text`, this is the `min` floor, not the measured size.
    /// The actual measured size is computed by the solver at layout time.
    pub fn min_size(&self) -> f32 {
        match self {
            Sizing::Fixed(size) => *size,
            Sizing::Fractional { min, .. } => *min,
            Sizing::Flex { min, .. } => *min,
            Sizing::Text { min, .. } => *min,
        }
    }
}

// ============================================================================
// Display Tiers
// ============================================================================

/// A named display mode activated when a slot's resolved size meets a threshold.
///
/// Tiers are evaluated from largest `min_size` to smallest. The first tier
/// whose `min_size` is <= the resolved size wins. If none match, the slot
/// has no active tier (consumer can treat as "collapsed content").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayTier<'a> {
    /// Name of this tier (e.g., "Full", "Mini").
    pub name: &'a str,
    /// Minimum size along the parent's main axis for this tier to be active.
    pub min_size: f32,
}

// ============================================================================
// Layout Nodes
// ============================================================================

/// A node in the declarative layout tree.
#[derive(Debug, Clone, Copy)]
pub enum LayoutNode<'a> {
    /// A leaf: a named region where the consumer renders content.
    Slot(SlotNode<'a>),
    /// A branch: arranges children along an axis.
    Container(ContainerNode<'a>),
}

impl<'a> LayoutNode<'a> {
    /// Construct a leaf node with default slot options.
    ///
    /// Use [`SlotNode::new`] when you need to chain slot-specific options such
    /// as collapse labels or display tiers before converting into a
    /// `LayoutNode`.
    pub const fn slot(id: &'a str, sizing: Sizing<'a>) -> Self {
        LayoutNode::Slot(SlotNode::new(id, sizing))
    }

    /// Construct a container node with default container options.
    ///
    /// Use [`ContainerNode::new`] when you need to chain options such as
    /// `auto_axis` or `divider_size` before converting into a `LayoutNode`.
    pub const fn container(
        id: &'a str,
        axis: Axis,
        sizing: Sizing<'a>,
        children: &'a [LayoutNode<'a>],
    ) -> Self {
        LayoutNode::Container(ContainerNode::new(id, axis, sizing, children))
    }

    /// Returns the node's unique identifier.
    pub fn id(&self) -> &'a str {
        match self {
            LayoutNode::Slot(s) => s.id,
            LayoutNode::Container(c) => c.id,
        }
    }

    /// Returns the node's sizing constraint.
    pub fn sizing(&self) -> Sizing<'a> {
        match self {
            LayoutNode::Slot(s) => s.sizing,
            LayoutNode::Container(c) => c.sizing,
        }
    }

    /// Returns true if this node can be collapsed.
    pub fn collapsible(&self) -> bool {
        match self {
            LayoutNode::Slot(s) => s.collapsible,
            LayoutNode::Container(_) => false,
        }
    }

    /// Returns the collapse priority (0.0 = first to collapse, 1.0 = never).
    pub fn priority(&self) -> f32 {
        match self {
            LayoutNode::Slot(s) => s.priority,
            LayoutNode::Container(_) => 1.0,
        }
    }
}

impl<'a> From<SlotNode<'a>> for LayoutNode<'a> {
    fn from(slot: SlotNode<'a>) -> Self {
        LayoutNode::Slot(slot)
    }
}

impl<'a> From<ContainerNode<'a>> for LayoutNode<'a> {
    fn from(container: ContainerNode<'a>) -> Self {
        LayoutNode::Container(container)
    }
}

/// A leaf node: a named slot where the consumer renders content.
#[derive(Debug, Clone, Copy)]
pub struct SlotNode<'a> {
    /// Unique identifier (e.g., "library", "queue", "header").
    pub id: &'a str,
    /// How this slot claims space.
    pub sizing: Sizing<'a>,
    /// Collapse priority: 0.0 = collapses first, 1.0 = never collapses.
    pub priority: f32,
    /// Whether this slot can be collapsed when space is insufficient.
    pub collapsible: bool,
    /// Display mode tiers based on resolved size (evaluated largest-first).
    pub display_tiers: &'a [DisplayTier<'a>],
    /// Tab label when this slot is collapsed. `None` = no tab.
    pub collapse_label: Option<&'a str>,
}

impl<'a> SlotNode<'a> {
    /// Construct a slot with the standard non-collapsible defaults.
    pub const fn new(id: &'a str, sizing: Sizing<'a>) -> Self {
        Self {
            id,
            sizing,
            priority: 1.0,
            collapsible: false,
            display_tiers: &[],
            collapse_label: None,
        }
    }

    /// Set the collapse priority.
    pub const fn priority(mut self, priority: f32) -> Self {
        self.priority = priority;
        self
    }

    /// Mark the slot as collapsible and set its collapse tab label.
    pub const fn collapsible(mut self, priority: f32, collapse_label: &'a str) -> Self {
        self.priority = priority;
        self.collapsible = true;
        self.collapse_label = Some(collapse_label);
        self
    }

    /// Set display tiers used by responsive renderers.
    pub const fn display_tiers(mut self, display_tiers: &'a [DisplayTier<'a>]) -> Self {
        self.display_tiers = display_tiers;
        self
    }

    /// Set or clear the collapse tab label without changing collapsibility.
    pub const fn collapse_label(mut self, collapse_label: Option<&'a str>) -> Self {
        self.collapse_label = collapse_label;
        self
    }

    /// Convert this slot into a layout node.
    pub const fn into_node(self) -> LayoutNode<'a> {
        LayoutNode::Slot(self)
    }
}

/// A container node: arranges children along an axis.
#[derive(Debug, Clone, Copy)]
pub struct ContainerNode<'a> {
    /// Unique identifier for this container.
    pub id: &'a str,
    /// Primary axis for child arrangement.
    pub axis: Axis,
    /// If `Some(threshold)`, the axis flips when the parent's width/height
    /// ratio crosses this threshold. At threshold=1.0, axis becomes Horizontal
    /// when width > height, Vertical otherwise.
    pub auto_axis: Option<f32>,
    /// How this container claims space within its parent.
    pub sizing: Sizing<'a>,
    /// Children, ordered along the main axis.
    pub children: &'a [LayoutNode<'a>],
    /// Pixel size to reserve between each pair of visible children (for dividers).
    pub divider_size: f32,
}

impl<'a> ContainerNode<'a> {
    /// Construct a container with no auto-axis switching or dividers.
    pub const fn new(
        id: &'a str,
        axis: Axis,
        sizing: Sizing<'a>,
        children: &'a [LayoutNode<'a>],
    ) -> Self {
        Self {
            id,
            axis,
            auto_axis: None,
            sizing,
            children,
            divider_size: 0.0,
        }
    }

    /// Flip the container axis when the available width/height crosses `threshold`.
    pub const fn auto_axis(mut self, threshold: f32) -> Self {
        self.auto_axis = Some(threshold);
        self
    }

    /// Reserve a fixed divider size between visible children.
    pub const fn divider_size(mut self, divider_size: f32) -> Self {
        self.divider_size = divider_size;
        self
    }

    /// Convert this container into a layout node.
    pub const fn into_node(self) -> LayoutNode<'a> {
        LayoutNode::Container(self)
    }
}

// ============================================================================
// User Preferences (solver input)
// ============================================================================

/// External state provided to the solver. The solver reads but never writes.
#[derive(Debug, Clone, Default)]
pub struct LayoutPreferences<'a> {
    /// Per-slot ratio overrides, keyed by (slot_id, parent_axis).
    /// When the solver resolves a `Fractional` slot, it looks here first.
    /// If not found, uses `Sizing::Fractional::initial`.
    pub ratios: &'a [(&'a str, Axis, f32)],
    /// Per-slot collapsed state from user toggle.
    /// If a slot's id appears here with `true`, and the slot is `collapsible`,
    /// the solver treats it as collapsed (0 size) regardless of available space.
    pub collapsed: &'a [(&'a str, bool)],
}

impl<'a> LayoutPreferences<'a> {
    /// Look up the user's ratio override for a slot in a given axis.
    pub fn ratio_for(&self, id: &str, axis: Axis) -> Option<f32> {
        self.ratios
            .iter()
            .find(|(slot_id, a, _)| *slot_id == id && *a == axis)
            .map(|(_, _, r)| *r)
    }

    /// Check if a slot is user-collapsed.
    pub fn is_collapsed(&self, id: &str) -> bool {
        self.collapsed
            .iter()
            .any(|(slot_id, collapsed)| *slot_id == id && *collapsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solve;

    #[test]
    fn slot_constructor_uses_non_collapsible_defaults() {
        let slot = SlotNode::new("main", Sizing::flex(100.0));

        assert_eq!(slot.id, "main");
        assert_eq!(slot.sizing, Sizing::flex(100.0));
        assert_eq!(slot.priority, 1.0);
        assert!(!slot.collapsible);
        assert!(slot.display_tiers.is_empty());
        assert_eq!(slot.collapse_label, None);
    }

    #[test]
    fn fluent_slot_options_set_collapse_and_tiers() {
        static TIERS: &[DisplayTier<'_>] = &[
            DisplayTier {
                name: "Full",
                min_size: 200.0,
            },
            DisplayTier {
                name: "Mini",
                min_size: 100.0,
            },
        ];

        let slot = SlotNode::new("rack", Sizing::fractional(0.3, 80.0))
            .display_tiers(TIERS)
            .collapsible(0.4, "Rack");

        assert_eq!(slot.priority, 0.4);
        assert!(slot.collapsible);
        assert_eq!(slot.display_tiers, TIERS);
        assert_eq!(slot.collapse_label, Some("Rack"));
    }

    #[test]
    fn container_constructors_use_default_options() {
        let children = [LayoutNode::slot("main", Sizing::flex(0.0))];
        let container = ContainerNode::new("root", Axis::Vertical, Sizing::flex(0.0), &children);

        assert_eq!(container.id, "root");
        assert_eq!(container.axis, Axis::Vertical);
        assert_eq!(container.auto_axis, None);
        assert_eq!(container.sizing, Sizing::flex(0.0));
        assert_eq!(container.children.len(), 1);
        assert_eq!(container.divider_size, 0.0);

        let node = LayoutNode::container("root", Axis::Vertical, Sizing::flex(0.0), &children);
        assert!(matches!(node, LayoutNode::Container(_)));
    }

    #[test]
    fn fluent_constructors_match_explicit_struct_layout() {
        let explicit_children = [
            LayoutNode::Slot(SlotNode {
                id: "sidebar",
                sizing: Sizing::fractional(0.25, 100.0),
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
        ];
        let explicit = LayoutNode::Container(ContainerNode {
            id: "root",
            axis: Axis::Horizontal,
            auto_axis: Some(1.0),
            sizing: Sizing::flex(0.0),
            children: &explicit_children,
            divider_size: 6.0,
        });

        let fluent_children = [
            SlotNode::new("sidebar", Sizing::fractional(0.25, 100.0))
                .collapsible(0.5, "Sidebar")
                .into(),
            LayoutNode::slot("main", Sizing::flex(200.0)),
        ];
        let fluent = ContainerNode::new(
            "root",
            Axis::Horizontal,
            Sizing::flex(0.0),
            &fluent_children,
        )
        .auto_axis(1.0)
        .divider_size(6.0)
        .into_node();

        let explicit_solved = solve(&explicit, 1000.0, 600.0, &LayoutPreferences::default());
        let fluent_solved = solve(&fluent, 1000.0, 600.0, &LayoutPreferences::default());

        for id in ["root", "sidebar", "main"] {
            let explicit = explicit_solved.find(id).unwrap();
            let fluent = fluent_solved.find(id).unwrap();
            assert_eq!(fluent.width, explicit.width, "width mismatch for {id}");
            assert_eq!(fluent.height, explicit.height, "height mismatch for {id}");
            assert_eq!(
                fluent.visible, explicit.visible,
                "visibility mismatch for {id}"
            );
        }
    }
}
