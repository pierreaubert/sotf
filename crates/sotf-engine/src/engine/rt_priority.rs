// ============================================================================
// Real-Time Thread Priority
// ============================================================================
//
// Platform-specific thread priority elevation for audio threads.
// Gracefully degrades if RT privileges are unavailable.

/// Priority level for audio threads.
#[derive(Debug, Clone, Copy)]
pub enum RtPriority {
    /// Reserved for a backend-owned hardware callback integration. The cpal
    /// playback feeder must not use this hard realtime policy.
    #[allow(
        dead_code,
        reason = "reserved for a future callback-owned backend hook"
    )]
    Playback,
    /// Elevated but not hard-RT (processing thread)
    Processing,
}

/// Attempt to elevate the calling thread's priority.
///
/// Returns `Ok(true)` if priority was successfully set, `Ok(false)` if the
/// platform doesn't support it, or `Err` on failure.
pub fn set_realtime_priority(
    level: RtPriority,
    audio_timing: Option<(u32, usize)>,
) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        set_priority_macos(level, audio_timing)
    }

    #[cfg(target_os = "linux")]
    {
        set_priority_linux(level)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (level, audio_timing);
        Ok(false)
    }
}

// ============================================================================
// macOS: QoS class for processing, time-constraint for playback
// ============================================================================

#[cfg(target_os = "macos")]
fn set_priority_macos(
    level: RtPriority,
    audio_timing: Option<(u32, usize)>,
) -> Result<bool, String> {
    match level {
        RtPriority::Playback => match set_time_constraint_priority_macos(audio_timing) {
            Ok(()) => {
                log::info!("[RT Priority] macOS: set THREAD_TIME_CONSTRAINT_POLICY for playback");
                Ok(true)
            }
            Err(err) => {
                log::warn!(
                    "[RT Priority] macOS: failed to set time-constraint policy ({err}), falling back to QoS"
                );
                set_qos_priority_macos(level)
            }
        },
        RtPriority::Processing => set_qos_priority_macos(level),
    }
}

#[cfg(target_os = "macos")]
fn set_qos_priority_macos(level: RtPriority) -> Result<bool, String> {
    // pthread_set_qos_class_self_np(qos_class, relative_priority)
    unsafe extern "C" {
        fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: i32) -> i32;
    }

    const QOS_CLASS_USER_INTERACTIVE: u32 = 0x21;

    let ret = unsafe { pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0) };
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

#[cfg(target_os = "macos")]
fn set_time_constraint_priority_macos(audio_timing: Option<(u32, usize)>) -> Result<(), String> {
    use std::mem::MaybeUninit;

    type BooleanT = i32;
    type IntegerT = i32;
    type KernReturnT = i32;
    type MachMsgTypeNumberT = u32;
    type MachPortT = u32;
    type ThreadPolicyFlavorT = i32;

    const KERN_SUCCESS: KernReturnT = 0;
    const MACH_PORT_NULL: MachPortT = 0;
    const THREAD_TIME_CONSTRAINT_POLICY: ThreadPolicyFlavorT = 2;
    const THREAD_TIME_CONSTRAINT_POLICY_COUNT: MachMsgTypeNumberT = 4;

    let audio_period_ns = audio_timing
        .and_then(|(sample_rate, frame_size)| audio_period_nanos(sample_rate, frame_size))
        .unwrap_or(2_900_000);
    let audio_computation_ns = (audio_period_ns / 2).max(1);

    #[repr(C)]
    struct MachTimebaseInfoData {
        numer: u32,
        denom: u32,
    }

    #[repr(C)]
    struct ThreadTimeConstraintPolicyData {
        period: u32,
        computation: u32,
        constraint: u32,
        preemptible: BooleanT,
    }

    unsafe extern "C" {
        static mach_task_self_: MachPortT;

        fn mach_thread_self() -> MachPortT;
        fn mach_port_deallocate(task: MachPortT, name: MachPortT) -> KernReturnT;
        fn mach_timebase_info(info: *mut MachTimebaseInfoData) -> KernReturnT;
        fn thread_policy_set(
            thread: MachPortT,
            flavor: ThreadPolicyFlavorT,
            policy_info: *mut IntegerT,
            count: MachMsgTypeNumberT,
        ) -> KernReturnT;
    }

    let mut timebase = MaybeUninit::<MachTimebaseInfoData>::uninit();
    let timebase_result = unsafe { mach_timebase_info(timebase.as_mut_ptr()) };
    if timebase_result != KERN_SUCCESS {
        return Err(format!("mach_timebase_info failed: {timebase_result}"));
    }
    let timebase = unsafe { timebase.assume_init() };

    let mut policy = ThreadTimeConstraintPolicyData {
        period: nanos_to_mach_absolute_time(audio_period_ns, timebase.numer, timebase.denom)?,
        computation: nanos_to_mach_absolute_time(
            audio_computation_ns,
            timebase.numer,
            timebase.denom,
        )?,
        constraint: nanos_to_mach_absolute_time(audio_period_ns, timebase.numer, timebase.denom)?,
        preemptible: 1,
    };

    let thread = unsafe { mach_thread_self() };
    if thread == MACH_PORT_NULL {
        return Err("mach_thread_self returned MACH_PORT_NULL".to_string());
    }

    let result = unsafe {
        thread_policy_set(
            thread,
            THREAD_TIME_CONSTRAINT_POLICY,
            &mut policy as *mut ThreadTimeConstraintPolicyData as *mut IntegerT,
            THREAD_TIME_CONSTRAINT_POLICY_COUNT,
        )
    };
    let _ = unsafe { mach_port_deallocate(mach_task_self_, thread) };

    if result == KERN_SUCCESS {
        Ok(())
    } else {
        Err(format!("thread_policy_set failed: {result}"))
    }
}

#[cfg(target_os = "macos")]
fn audio_period_nanos(sample_rate: u32, frame_size: usize) -> Option<u64> {
    if sample_rate == 0 || frame_size == 0 {
        return None;
    }
    let nanos = (frame_size as u128)
        .saturating_mul(1_000_000_000)
        .div_ceil(sample_rate as u128);
    u64::try_from(nanos).ok()
}

#[cfg(target_os = "macos")]
fn nanos_to_mach_absolute_time(nanos: u64, numer: u32, denom: u32) -> Result<u32, String> {
    if numer == 0 || denom == 0 {
        return Err("invalid mach timebase ratio".to_string());
    }

    let absolute = (nanos as u128)
        .saturating_mul(denom as u128)
        .checked_div(numer as u128)
        .ok_or_else(|| "invalid mach timebase ratio".to_string())?;

    u32::try_from(absolute).map_err(|_| format!("mach absolute time out of range: {absolute}"))
}

#[cfg(all(test, target_os = "macos"))]
mod macos_tests {
    use super::{audio_period_nanos, nanos_to_mach_absolute_time};

    #[test]
    fn nanos_to_mach_absolute_time_converts_and_rejects_invalid_timebase() {
        assert_eq!(nanos_to_mach_absolute_time(1_000, 1, 1).unwrap(), 1_000);
        assert_eq!(nanos_to_mach_absolute_time(1_000, 2, 1).unwrap(), 500);
        assert_eq!(nanos_to_mach_absolute_time(1_000, 1, 2).unwrap(), 2_000);
        assert!(nanos_to_mach_absolute_time(1_000, 0, 1).is_err());
        assert!(nanos_to_mach_absolute_time(1_000, 1, 0).is_err());
    }

    #[test]
    fn audio_period_uses_configured_frame_size_and_sample_rate() {
        assert_eq!(audio_period_nanos(48_000, 480), Some(10_000_000));
        assert_eq!(audio_period_nanos(44_100, 128), Some(2_902_495));
        assert_eq!(audio_period_nanos(0, 128), None);
    }
}

// ============================================================================
// Linux: SCHED_FIFO for playback, SCHED_RR for processing
// ============================================================================

#[cfg(target_os = "linux")]
fn set_priority_linux(level: RtPriority) -> Result<bool, String> {
    use libc::{SCHED_FIFO, SCHED_RR, sched_param, sched_setscheduler};

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
            if policy == SCHED_FIFO {
                "SCHED_FIFO"
            } else {
                "SCHED_RR"
            },
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
