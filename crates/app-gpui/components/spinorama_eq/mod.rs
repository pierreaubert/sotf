//! Spinorama EQ Screen
//!
//! Multi-step wizard for speaker EQ optimization using spinorama.org data:
//! 1. Select Speaker - Search and select speaker from spinorama.org API
//! 2. Configure - Optimization parameters and mode selection
//! 3. Optimize - Run optimization with progress display
//! 4. Review - View results, apply to playback, export

mod step_1_select;
mod step_2_configure;
mod step_3_review;
mod step_4_export;

mod misc;
mod spawn;
mod types;
