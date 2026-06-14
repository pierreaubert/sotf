//! Room EQ configuration and serialization tests.

#[path = "room_eq_config_tests/build.rs"]
mod build;
#[path = "room_eq_config_tests/make.rs"]
mod make;
#[path = "room_eq_config_tests/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "room_eq_config_tests/tests.rs"]
mod tests;
#[path = "room_eq_config_tests/types.rs"]
mod types;
