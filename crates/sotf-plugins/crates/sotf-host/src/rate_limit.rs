// Rate-limited logging helpers for audio-thread error paths.
//
// Goal: when a recoverable error happens repeatedly on the audio thread,
// log it once, then suppress further logs of the same kind for a short
// interval. The check itself is allocation-free and lock-free — it relies
// on a static AtomicU64 storing the next-allowed timestamp in nanoseconds.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Returns the elapsed monotonic time as nanoseconds since process start.
#[inline]
fn now_ns() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(Instant::now);
    Instant::now().saturating_duration_since(*start).as_nanos() as u64
}

/// Returns `true` at most once per `interval_ns` for the given gate.
///
/// The gate is a `&'static AtomicU64` storing the next-allowed timestamp.
/// Use [`rate_limited_log!`] to declare a fresh gate at the call site.
#[inline]
pub fn allow(gate: &AtomicU64, interval_ns: u64) -> bool {
    let now = now_ns();
    let prev = gate.load(Ordering::Relaxed);
    if now < prev {
        return false;
    }
    // Compare-exchange to avoid two threads both logging.
    gate.compare_exchange(
        prev,
        now.saturating_add(interval_ns),
        Ordering::Relaxed,
        Ordering::Relaxed,
    )
    .is_ok()
}

/// Log at `level` no more than once per `secs` seconds for this call site.
///
/// Backed by a per-call-site `AtomicU64` gate, so different invocations do
/// not share rate-limit state. Safe to call from the audio thread: no
/// allocation, no lock.
#[macro_export]
macro_rules! rate_limited_log {
    ($level:ident, $secs:expr, $($arg:tt)+) => {{
        static GATE: ::std::sync::atomic::AtomicU64 = ::std::sync::atomic::AtomicU64::new(0);
        if $crate::rate_limit::allow(&GATE, ($secs as u64).saturating_mul(1_000_000_000)) {
            ::log::$level!($($arg)+);
        }
    }};
}
