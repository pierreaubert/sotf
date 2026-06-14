mod fixtures;

#[path = "database_tests/test.rs"]
mod test;
#[cfg(test)]
#[path = "database_tests/tests.rs"]
mod tests;
