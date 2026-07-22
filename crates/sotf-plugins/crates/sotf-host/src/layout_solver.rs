//! Layout Constraint Solver
//!
//! Lightweight greedy solver that decides which columns are visible vs. collapsed
//! to tabs based on available screen space. Platform-agnostic — same solver runs
//! on GPUI, SwiftUI, Android.
//!
//! Algorithm:
//! 1. If width < VERTICAL_THRESHOLD → vertical mode, all collapsible columns become tabs
//! 2. Reserve min_width for Main (never collapses)
//! 3. Greedily fit remaining columns by priority (highest first)
//! 4. Columns that don't fit become tabs
//! 5. Distribute leftover space to Main (flex)
//! 6. Determine internal adaptations (slider height, group direction, viz visibility)

mod consts;
mod group;
mod misc;
mod solve;
mod solved_layout;
#[cfg(test)]
mod tests;
mod types;

pub use consts::*;
pub use group::*;
pub use solve::*;
pub use solved_layout::*;
pub use types::*;
