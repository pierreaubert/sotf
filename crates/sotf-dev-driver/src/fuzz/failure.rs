use std::cmp::Ordering;

use super::model::{FailureClass, ResourceSample};

pub const MEMORY_SLOPE_BYTES_PER_MINUTE: f64 = 1024.0 * 1024.0;
pub const MEMORY_MIN_RETAINED_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeResult {
    Responsive,
    TimedOut,
    ProcessExited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HangEvidence {
    pub action_timed_out: bool,
    pub live: ProbeResult,
    pub snapshot: ProbeResult,
    pub consecutive_misses: u8,
    pub process_progressed: bool,
}

pub fn classify_hang(evidence: HangEvidence) -> Option<FailureClass> {
    if !evidence.action_timed_out || evidence.consecutive_misses < 3 {
        return None;
    }
    match (evidence.live, evidence.snapshot) {
        (ProbeResult::ProcessExited, _) | (_, ProbeResult::ProcessExited) => None,
        (ProbeResult::Responsive, ProbeResult::TimedOut) => Some(FailureClass::MainLoopStall),
        (ProbeResult::TimedOut, _) if !evidence.process_progressed => {
            Some(FailureClass::WholeProcessHang)
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryClassification {
    pub slope_bytes_per_minute: f64,
    pub baseline_bytes: u64,
    pub final_bytes: u64,
    pub retained_growth_bytes: u64,
    pub retained_threshold_bytes: u64,
    pub suspected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetainedCountGrowth {
    pub baseline: u64,
    pub final_value: u64,
    pub retained_growth: u64,
    pub absolute_budget: u64,
    pub suspected: bool,
}

pub fn classify_memory_growth(samples: &[ResourceSample]) -> Option<MemoryClassification> {
    let points: Vec<_> = samples
        .iter()
        .filter_map(|sample| {
            sample
                .rss_bytes
                .map(|rss| (sample.monotonic_ms as f64 / 60_000.0, rss as f64))
        })
        .collect();
    if points.len() < 6 {
        return None;
    }
    let window = (points.len() / 5).max(3).min(points.len() / 2);
    let baseline = median_u64(
        &points[..window]
            .iter()
            .map(|(_, rss)| *rss as u64)
            .collect::<Vec<_>>(),
    );
    let final_bytes = median_u64(
        &points[points.len() - window..]
            .iter()
            .map(|(_, rss)| *rss as u64)
            .collect::<Vec<_>>(),
    );
    let retained_growth_bytes = final_bytes.saturating_sub(baseline);
    let retained_threshold_bytes = MEMORY_MIN_RETAINED_BYTES.max(baseline / 10);
    let slope_bytes_per_minute = theil_sen_slope(&points)?;
    Some(MemoryClassification {
        slope_bytes_per_minute,
        baseline_bytes: baseline,
        final_bytes,
        retained_growth_bytes,
        retained_threshold_bytes,
        suspected: slope_bytes_per_minute > MEMORY_SLOPE_BYTES_PER_MINUTE
            && retained_growth_bytes > retained_threshold_bytes,
    })
}

pub fn theil_sen_slope(points: &[(f64, f64)]) -> Option<f64> {
    let mut slopes = Vec::new();
    for (left_index, (left_x, left_y)) in points.iter().enumerate() {
        for (right_x, right_y) in &points[left_index + 1..] {
            let delta_x = right_x - left_x;
            if delta_x > 0.0 && delta_x.is_finite() {
                let slope = (right_y - left_y) / delta_x;
                if slope.is_finite() {
                    slopes.push(slope);
                }
            }
        }
    }
    median_f64(&mut slopes)
}

pub fn classify_retained_count_growth(
    values: &[u64],
    absolute_budget: u64,
) -> Option<RetainedCountGrowth> {
    // Keep a distinct warm baseline plus three final windows. Reusing the
    // baseline as the first final window makes sustained growth impossible.
    if values.len() < 12 {
        return None;
    }
    let window = values.len() / 4;
    let baseline = median_u64(&values[..window]);
    let a = median_u64(&values[values.len() - window * 3..values.len() - window * 2]);
    let b = median_u64(&values[values.len() - window * 2..values.len() - window]);
    let c = median_u64(&values[values.len() - window..]);
    Some(RetainedCountGrowth {
        baseline,
        final_value: c,
        retained_growth: c.saturating_sub(baseline),
        absolute_budget,
        suspected: a > baseline.saturating_add(absolute_budget) && a <= b && b <= c,
    })
}

pub fn sustained_retained_growth(values: &[u64], absolute_budget: u64) -> bool {
    classify_retained_count_growth(values, absolute_budget).is_some_and(|growth| growth.suspected)
}

fn median_u64(values: &[u64]) -> u64 {
    let mut values = values.to_vec();
    values.sort_unstable();
    values[values.len() / 2]
}

fn median_f64(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    Some(values[values.len() / 2])
}

pub fn normalize_signature(text: &str, run_dir: Option<&str>) -> String {
    let mut normalized = text.replace("0x", "addr:");
    if let Some(run_dir) = run_dir {
        normalized = normalized.replace(run_dir, "<run-dir>");
    }
    normalized
        .split_whitespace()
        .map(|token| {
            if token.chars().all(|character| character.is_ascii_digit()) {
                "<n>"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn sample(minute: u64, mib: u64) -> ResourceSample {
        ResourceSample {
            monotonic_ms: minute * 60_000,
            rss_bytes: Some(mib * 1024 * 1024),
            virtual_bytes: None,
            cpu_percent: None,
            cpu_time_ms: None,
            threads: None,
            descriptors_or_handles: None,
            children: None,
            unavailable: BTreeMap::new(),
        }
    }

    #[test]
    fn memory_requires_both_slope_and_retained_growth() {
        let leaking: Vec<_> = (0..20)
            .map(|minute| sample(minute, 100 + minute * 3))
            .collect();
        assert!(classify_memory_growth(&leaking).unwrap().suspected);
        let small: Vec<_> = (0..20).map(|minute| sample(minute, 100 + minute)).collect();
        assert!(!classify_memory_growth(&small).unwrap().suspected);
        let spike: Vec<_> = (0..20)
            .map(|minute| sample(minute, if minute == 10 { 500 } else { 100 }))
            .collect();
        assert!(!classify_memory_growth(&spike).unwrap().suspected);
    }

    #[test]
    fn distinguishes_main_loop_stall_and_process_hang() {
        assert_eq!(
            classify_hang(HangEvidence {
                action_timed_out: true,
                live: ProbeResult::Responsive,
                snapshot: ProbeResult::TimedOut,
                consecutive_misses: 3,
                process_progressed: false,
            }),
            Some(FailureClass::MainLoopStall)
        );
        assert_eq!(
            classify_hang(HangEvidence {
                action_timed_out: true,
                live: ProbeResult::TimedOut,
                snapshot: ProbeResult::TimedOut,
                consecutive_misses: 3,
                process_progressed: false,
            }),
            Some(FailureClass::WholeProcessHang)
        );
    }

    #[test]
    fn retained_count_growth_uses_a_separate_baseline_and_three_final_windows() {
        let retained = [1, 1, 1, 4, 4, 4, 5, 5, 5, 6, 6, 6];
        let classification = classify_retained_count_growth(&retained, 2).unwrap();
        assert!(classification.suspected);
        assert_eq!(classification.baseline, 1);
        assert_eq!(classification.final_value, 6);

        let transient = [1, 1, 1, 8, 8, 8, 1, 1, 1, 1, 1, 1];
        assert!(!sustained_retained_growth(&transient, 2));
        assert!(classify_retained_count_growth(&retained[..11], 2).is_none());
    }
}
