use super::consts::HAL_INPUT_RUNAWAY_PEAK_LIMIT;
use super::types::HalInputGuardTrip;

#[cfg_attr(not(all(target_os = "macos", feature = "hal")), allow(dead_code))]
pub(super) fn inspect_hal_input_block(samples: &[f32]) -> Option<HalInputGuardTrip> {
    let mut peak = 0.0f32;
    let mut invalid_samples = 0usize;
    let mut over_limit_samples = 0usize;

    for &sample in samples {
        if !sample.is_finite() {
            invalid_samples += 1;
            continue;
        }

        let abs_sample = sample.abs();
        peak = peak.max(abs_sample);
        if abs_sample > HAL_INPUT_RUNAWAY_PEAK_LIMIT {
            over_limit_samples += 1;
        }
    }

    if invalid_samples > 0 || over_limit_samples > 0 {
        Some(HalInputGuardTrip {
            peak,
            invalid_samples,
            over_limit_samples,
        })
    } else {
        None
    }
}

#[cfg_attr(not(all(target_os = "macos", feature = "hal")), allow(dead_code))]
pub(super) fn guard_hal_input_block(samples: &mut [f32]) -> Option<HalInputGuardTrip> {
    let trip = inspect_hal_input_block(samples)?;
    samples.fill(0.0);
    Some(trip)
}
