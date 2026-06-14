use std::cell::Cell;

thread_local! {
    static ALLOC_COUNT: Cell<usize> = const { Cell::new(0) };
    static COUNTING_ENABLED: Cell<bool> = const { Cell::new(false) };
}

#[path = "realtime_allocation_tests/consts.rs"]
mod consts;
#[path = "realtime_allocation_tests/counting_alloc.rs"]
mod counting_alloc;
#[path = "realtime_allocation_tests/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "realtime_allocation_tests/tests.rs"]
mod tests;
