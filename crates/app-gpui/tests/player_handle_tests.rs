//! Exercise the production actor without loading the GPUI render tree in test mode.
//! The library's minimal unit-test harness excludes `app`, so its private actor
//! tests must be compiled explicitly by this integration target.
#[allow(dead_code)]
#[path = "../app/player_handle.rs"]
mod player_handle;
