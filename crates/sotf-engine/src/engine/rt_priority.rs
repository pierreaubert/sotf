// ============================================================================
// Real-Time Thread Priority
// ============================================================================
//
// Platform-specific thread priority elevation for audio threads.
// Gracefully degrades if RT privileges are unavailable.

/// Priority level for audio threads.
#[derive(Debug, Clone, Copy)]
pub enum RtPriority {
    /// Highest priority: time-constraint / SCHED_FIFO (playback thread)
    Playback,
    /// Elevated but not hard-RT (processing thread)
    Processing,
}

/// Attempt to elevate the calling thread's priority.
///
/// Returns `Ok(true)` if priority was successfully set, `Ok(false)` if the
/// platform doesn't support it, or `Err` on failure.
pub fn set_realtime_priority(level: RtPriority) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        set_priority_macos(level)
    }

    #[cfg(target_os = "linux")]
    {
        set_priority_linux(level)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = level;
        Ok(false)
    }
}

// ============================================================================
// macOS: QoS class for processing, time-constraint for playback
// ============================================================================

#[cfg(target_os = "macos")]
fn set_priority_macos(level: RtPriority) -> Result<bool, String> {
    // Use QoS class for both levels — safer and doesn't require root.
    // QOS_CLASS_USER_INTERACTIVE is the highest non-RT class.
    // For true RT, we'd use thread_policy_set with THREAD_TIME_CONSTRAINT_POLICY,
    // but that requires specific period/computation/constraint values tied to
    // the audio callback timing, which the engine thread doesn't have.
    let qos_class = match level {
        RtPriority::Playback => 0x21,  // QOS_CLASS_USER_INTERACTIVE
        RtPriority::Processing => 0x21, // QOS_CLASS_USER_INTERACTIVE
    };

    // pthread_set_qos_class_self_np(qos_class, relative_priority)
    unsafe extern "C" {
        fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: i32) -> i32;
    }

    let ret = unsafe { pthread_set_qos_class_self_np(qos_class, 0) };
    if ret == 0 {
        log::info!(
            "[RT Priority] macOS: set QoS class to USER_INTERACTIVE for {:?}",
            level
        );
        Ok(true)
    } else {
        log::warn!(
            "[RT Priority] macOS: failed to set QoS class (errno={}), continuing at default priority",
            ret
        );
        Ok(false)
    }
}

// ============================================================================
// Linux: SCHED_FIFO for playback, SCHED_RR for processing
// ============================================================================

#[cfg(target_os = "linux")]
fn set_priority_linux(level: RtPriority) -> Result<bool, String> {
    use libc::{sched_param, sched_setscheduler, SCHED_FIFO, SCHED_RR};

    let (policy, priority) = match level {
        RtPriority::Playback => (SCHED_FIFO, 70),
        RtPriority::Processing => (SCHED_RR, 50),
    };

    let param = sched_param {
        sched_priority: priority,
    };

    let ret = unsafe { sched_setscheduler(0, policy, &param) };
    if ret == 0 {
        log::info!(
            "[RT Priority] Linux: set {:?} to policy={}, priority={}",
            level,
            if policy == SCHED_FIFO { "SCHED_FIFO" } else { "SCHED_RR" },
            priority
        );
        Ok(true)
    } else {
        let errno = std::io::Error::last_os_error();
        log::warn!(
            "[RT Priority] Linux: failed to set scheduler ({:?}), continuing at default priority. \
             Hint: run with CAP_SYS_NICE or as root for RT priority.",
            errno
        );
        Ok(false)
    }
}
