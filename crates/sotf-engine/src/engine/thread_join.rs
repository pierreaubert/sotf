//! Small helper for joining a thread with a timeout.
//!
//! This prevents a stuck CPAL/CoreAudio callback from blocking the test runner
//! (or the application shutdown path) indefinitely. If the timeout fires we log
//! a warning and leave the thread detached.

use std::thread::JoinHandle;
use std::time::Duration;

/// Join `handle`, waiting at most `timeout`.
///
/// Returns `Ok(())` if the thread finished in time, or `Err(())` if the join
/// timed out. In the timeout case the watcher thread keeps trying to join the
/// original thread, which is therefore leaked rather than blocking the caller.
pub fn join_timeout(handle: JoinHandle<()>, timeout: Duration) -> Result<(), ()> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(handle.join());
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) => {
            log::warn!("Thread being joined has panicked");
            Ok(())
        }
        Err(_) => {
            log::warn!(
                "Thread join timed out after {:?}; leaving thread detached",
                timeout
            );
            Err(())
        }
    }
}
