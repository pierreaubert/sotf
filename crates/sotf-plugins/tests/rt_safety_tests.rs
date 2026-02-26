#[cfg(test)]
mod tests {
    use sotf_plugins::{CountingAlloc, assert_no_allocs};

    #[global_allocator]
    static A: CountingAlloc = CountingAlloc;

    #[test]
    #[should_panic(expected = "allocations detected")]
    fn test_allocation_fails() {
        assert_no_allocs("test_allocation", || {
            let _v: Vec<i32> = vec![1, 2, 3];
        });
    }

    #[test]
    fn test_no_allocation_passes() {
        assert_no_allocs("test_no_allocation", || {
            let _x = 1 + 1;
        });
    }
}
