//! Declarative layout types.
//!
//! Platform-agnostic data types for describing layout trees. No rendering code,
//! no framework dependencies. Consumed by the solver to produce resolved geometry.

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
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sizing {
    /// Fixed size in pixels. Always gets exactly this much space.
    /// Use for: headers, footers, toolbars, fixed-width sidebars.
    Fixed(f32),

    /// Fractional: claims a fraction of remaining space (after fixed allocations).
    /// `initial` is the default ratio (0.0..=1.0); user preferences can override it.
    /// `min` and `max` are hard pixel bounds.
    /// Use for: resizable side panels (library, rack).
    Fractional {
        initial: f32,
        min: f32,
        max: f32,
    },

    /// Flex: takes all remaining space after siblings are allocated.
    /// If multiple Flex siblings exist, they split remaining space by weight.
    /// `min` is the absolute minimum in pixels.
    /// Use for: main content areas (queue panel, plugin main column).
    Flex {
        min: f32,
        weight: f32,
    },
}

impl Sizing {
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
    pub fn min_size(&self) -> f32 {
        match self {
            Sizing::Fixed(size) => *size,
            Sizing::Fractional { min, .. } => *min,
            Sizing::Flex { min, .. } => *min,
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
    /// Returns the node's unique identifier.
    pub fn id(&self) -> &'a str {
        match self {
            LayoutNode::Slot(s) => s.id,
            LayoutNode::Container(c) => c.id,
        }
    }

    /// Returns the node's sizing constraint.
    pub fn sizing(&self) -> Sizing {
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

/// A leaf node: a named slot where the consumer renders content.
#[derive(Debug, Clone, Copy)]
pub struct SlotNode<'a> {
    /// Unique identifier (e.g., "library", "queue", "header").
    pub id: &'a str,
    /// How this slot claims space.
    pub sizing: Sizing,
    /// Collapse priority: 0.0 = collapses first, 1.0 = never collapses.
    pub priority: f32,
    /// Whether this slot can be collapsed when space is insufficient.
    pub collapsible: bool,
    /// Display mode tiers based on resolved size (evaluated largest-first).
    pub display_tiers: &'a [DisplayTier<'a>],
    /// Tab label when this slot is collapsed. `None` = no tab.
    pub collapse_label: Option<&'a str>,
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
    pub sizing: Sizing,
    /// Children, ordered along the main axis.
    pub children: &'a [LayoutNode<'a>],
    /// Pixel size to reserve between each pair of visible children (for dividers).
    pub divider_size: f32,
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
