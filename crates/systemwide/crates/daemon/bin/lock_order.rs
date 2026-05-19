//! Lock-order invariant for the daemon.
//!
//! The daemon holds three coarse `parking_lot::Mutex`-protected objects
//! that can be acquired together: `driver_manager`, `manager`, and a
//! handful of small per-state mutexes (`selected_device`, `current_plugins`,
//! `current_input_channels`, `current_output_channels`, etc.).
//!
//! **The canonical lock order is:**
//!
//! 1. `driver_manager`
//! 2. `manager`
//! 3. per-state mutexes (`selected_device`, `current_plugins`, ...)
//!
//! The config-watcher thread acquires `driver_manager -> manager`, and
//! every IPC handler that touches both follows the same order. Holding
//! `manager` while trying to acquire `driver_manager` is a deadlock
//! waiting to happen.
//!
//! Historically the invariant was documented only in line comments next
//! to `handle_stop` and `handle_load_plugins_with_channels`. The helpers
//! in this module provide a runtime *assist* (not a static guarantee):
//! they call `try_lock` first, log a warning + caller location when the
//! lock is already held, and only then block. This turns silent
//! lock-order violations into noisy log lines we can spot in production
//! and reproduces nicely in tests.
//!
//! These helpers are intentionally thin -- a heavyweight typed `LockOrder`
//! state machine would touch every call site in the daemon. Surfacing
//! contention loudly is the high-value, low-blast-radius change.
use std::time::Duration;

use parking_lot::{Mutex, MutexGuard};

/// Try to acquire `mutex` without blocking. If it is contended, log a
/// warning identifying the caller (file:line) and the lock name, then
/// fall back to a bounded blocking wait.
///
/// Use this at sites that are part of the documented `driver_manager ->
/// manager -> per-state` chain. The warning is the signal that a future
/// contributor introduced a path where two threads grab the same lock in
/// conflicting orders -- it does not by itself prove a deadlock, but
/// when combined with the printed call sites it is enough to triage one.
///
/// Note: `parking_lot` has a `deadlock_detection` feature that does
/// global detection. We avoid enabling it project-wide (it adds overhead
/// to every lock). This helper is targeted, opt-in, and zero-cost when
/// uncontended.
#[track_caller]
pub fn lock_with_order_warning<'a, T>(mutex: &'a Mutex<T>, lock_name: &str) -> MutexGuard<'a, T> {
    if let Some(guard) = mutex.try_lock() {
        return guard;
    }
    let caller = std::panic::Location::caller();
    log::warn!(
        "lock contention on '{}' at {}:{} (caller is blocked; lock held by another thread)",
        lock_name,
        caller.file(),
        caller.line()
    );
    match mutex.try_lock_for(Duration::from_secs(5)) {
        Some(g) => g,
        None => {
            log::error!(
                "lock '{}' at {}:{} still held after 5s -- possible deadlock; blocking indefinitely",
                lock_name,
                caller.file(),
                caller.line()
            );
            mutex.lock()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn lock_with_order_warning_uncontended() {
        let m: Arc<Mutex<i32>> = Arc::new(Mutex::new(7));
        let g = lock_with_order_warning(&m, "test_uncontended");
        assert_eq!(*g, 7);
    }

    #[test]
    fn lock_with_order_warning_contended_logs_and_eventually_returns() {
        let m: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
        let m_clone = Arc::clone(&m);

        let t = std::thread::spawn(move || {
            let g = m_clone.lock();
            std::thread::sleep(Duration::from_millis(100));
            drop(g);
        });

        std::thread::sleep(Duration::from_millis(10));

        let g = lock_with_order_warning(&m, "test_contended");
        assert_eq!(*g, 0);
        drop(g);
        t.join().expect("spawned holder thread panicked");
    }
}
