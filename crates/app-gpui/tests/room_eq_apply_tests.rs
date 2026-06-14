//! Room-EQ "Apply to Rack" behavior tests.
//!
//! These tests are the **regression harness for Issue 6**: the optimizer
//! produces two distinct correction stages (a broadband pre-correction
//! and a main room-correction), and both must land in the plugin rack as
//! **two separately named EQ plugins** — not merged into one.
//!
//! The failing tests were written BEFORE the fix, using the OLD
//! `simulate_save_to_rack` helper (see `room_eq_config_tests.rs`) to
//! demonstrate the bug first, then asserting the new
//! `upsert_named_room_eq_plugins` function produces the correct shape.
//!
//! Naming contract the UI surfaces to end users:
//!   - Main correction  → "Room EQ"       (`max_filters = 10`)
//!   - Broadband stage  → "Broadband EQ"  (`max_filters = 4`)
//!
//! Both run in `per_channel_mode = true`.

#[path = "room_eq_apply_tests/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "room_eq_apply_tests/tests.rs"]
mod tests;
