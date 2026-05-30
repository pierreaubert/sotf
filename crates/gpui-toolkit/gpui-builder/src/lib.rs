//! Generic constraint-based layout solver.
//!
//! Platform-agnostic layout system that resolves a tree of `LayoutNode`
//! declarations into concrete pixel sizes. Supports:
//!
//! - **Hard constraints** (`Sizing::Fixed`) — headers, footers, toolbars
//! - **Soft constraints** (`Sizing::Fractional`, `Sizing::Flex`) — resizable panels
//! - **Priority-based collapse** — lowest-priority panels collapse first
//! - **Auto-axis switching** — horizontal/vertical based on aspect ratio
//! - **Display tiers** — panels report their active display mode based on size
//! - **User preferences** — ratio overrides and manual collapse state
//!
//! # Usage
//!
//! ```rust
//! use gpui_builder::{solve, LayoutNode, ContainerNode, SlotNode, Sizing, Axis, LayoutPreferences};
//!
//! let children = [
//!     LayoutNode::Slot(SlotNode {
//!         id: "header",
//!         sizing: Sizing::Fixed(40.0),
//!         priority: 1.0,
//!         collapsible: false,
//!         display_tiers: &[],
//!         collapse_label: None,
//!     }),
//!     LayoutNode::Slot(SlotNode {
//!         id: "content",
//!         sizing: Sizing::flex(200.0),
//!         priority: 1.0,
//!         collapsible: false,
//!         display_tiers: &[],
//!         collapse_label: None,
//!     }),
//! ];
//!
//! let root = LayoutNode::Container(ContainerNode {
//!     id: "root",
//!     axis: Axis::Vertical,
//!     auto_axis: None,
//!     sizing: Sizing::flex(0.0),
//!     children: &children,
//!     divider_size: 0.0,
//! });
//!
//! let solved = solve(&root, 1200.0, 800.0, &LayoutPreferences::default());
//! assert_eq!(solved.find("header").unwrap().height, 40.0);
//! assert_eq!(solved.find("content").unwrap().height, 760.0);
//! ```

pub mod compat;
#[macro_use]
mod macros;
pub mod plugin_chassis;
pub mod solved;
pub mod solver;
pub mod types;

// Re-exports for convenience
pub use solved::SolvedNode;
pub use solver::solve;
pub use types::{
    Axis, ContainerNode, DisplayTier, LayoutNode, LayoutPreferences, Sizing, SlotNode,
};

// gpui-pretext re-exports for text-measured sizing
pub use gpui_pretext::{
    EngineProfile as TextEngineProfile, PrepareOptions as TextPrepareOptions, TextMeasure,
};

// Compat re-exports
pub use compat::{
    ColumnRole, GroupDirection, KnobSize, Orientation, PluginAdaptations, PluginColumnConstraint,
    PluginLayoutThresholds, PluginLayoutTree, plugin_adaptations,
};

// Plugin chassis re-exports
pub use plugin_chassis::{
    ChassisLayout, FooterSpec, HeaderSpec, KnobSlot, RowSpec, SectionSpec, SolvedChassis,
    SolvedSection,
};
