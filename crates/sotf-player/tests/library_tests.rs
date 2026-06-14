mod fixtures;

#[path = "library_tests/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "library_tests/tests.rs"]
mod tests;
#[path = "library_tests/write.rs"]
mod write;
