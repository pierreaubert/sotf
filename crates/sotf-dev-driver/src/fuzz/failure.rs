use std::cmp::Ordering;

use super::model::{Action, ActionClass, FailureClass, Observation, ResourceSample};

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
        // Exit precedence: an exited process classifies as exit/signal, never hang.
        (ProbeResult::ProcessExited, _) | (_, ProbeResult::ProcessExited) => None,
        (ProbeResult::Responsive, ProbeResult::TimedOut) => Some(FailureClass::MainLoopStall),
        (ProbeResult::TimedOut, _) if !evidence.process_progressed => {
            Some(FailureClass::WholeProcessHang)
        }
        _ => None,
    }
}

/// Shared panic/error log gate used by capture and replay. Matches Rust panic
/// text and uppercase `ERROR` level tokens; lowercase "error" inside prose
/// (for example `errors=0`) does not trip the gate.
pub fn is_panic_or_error_log(line: &str) -> bool {
    line.contains("panicked at") || line.contains("panic!") || line.contains("ERROR")
}

/// One failure candidate derived from an observation. `signature` is raw text;
/// callers normalize it with `normalize_signature`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedFailure {
    pub class: FailureClass,
    pub signature: String,
    pub evidence: Vec<String>,
}

/// Classify an observation into failure candidates in the canonical priority
/// order shared by capture (supervisor) and replay (commands): exit, signal,
/// valid-action rejection, then panic/error log. The observation's
/// `failure_candidate` is intentionally excluded here; both call sites append
/// it last so the first candidate is comparable across capture and replay.
pub fn observation_failures(action: &Action, observation: &Observation) -> Vec<ObservedFailure> {
    let mut found = Vec::new();
    if !observation.process.alive {
        found.push(ObservedFailure {
            class: FailureClass::UnexpectedExit,
            signature: "process exited unexpectedly".into(),
            evidence: vec![],
        });
    }
    if let Some(signal) = &observation.process.signal_or_exception {
        found.push(ObservedFailure {
            class: FailureClass::SignalOrException,
            signature: signal.clone(),
            evidence: vec![signal.clone()],
        });
    }
    if action.class == ActionClass::StateValid
        && action.precondition_satisfied
        && observation.reply.as_ref().is_some_and(|reply| !reply.ok)
    {
        found.push(ObservedFailure {
            class: FailureClass::ValidActionRejection,
            signature: action.id.clone(),
            evidence: observation
                .reply
                .as_ref()
                .and_then(|reply| reply.error.clone())
                .into_iter()
                .collect(),
        });
    }
    if let Some(line) = observation
        .new_logs
        .iter()
        .find(|line| is_panic_or_error_log(line))
    {
        found.push(ObservedFailure {
            class: FailureClass::PanicOrErrorLog,
            signature: line.clone(),
            evidence: vec![line.clone()],
        });
    }
    found
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

/// Number of leading samples discarded as startup/warmup before the memory
/// classifier chooses its steady-state baseline window (design: "Portable
/// resource and memory sampling").
pub const MEMORY_WARMUP_SAMPLES: usize = 5;

pub fn classify_memory_growth(
    samples: &[ResourceSample],
    warmup_samples: usize,
) -> Option<MemoryClassification> {
    let points: Vec<_> = samples
        .iter()
        .skip(warmup_samples)
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

/// Maximum number of points fed to the Theil-Sen estimator. The estimator is
/// O(n²) over all pairs, so longer runs are deterministically downsampled
/// (raw samples are preserved in metrics.ndjson by the supervisor).
pub const THEIL_SEN_MAX_POINTS: usize = 2_048;

pub fn theil_sen_slope(points: &[(f64, f64)]) -> Option<f64> {
    let points = downsample_points(points, THEIL_SEN_MAX_POINTS);
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

/// Deterministic even-stride downsample preserving order and endpoints.
fn downsample_points(points: &[(f64, f64)], max_points: usize) -> Vec<(f64, f64)> {
    if points.len() <= max_points {
        return points.to_vec();
    }
    (0..max_points)
        .map(|index| points[index * points.len() / max_points])
        .collect()
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
    let mut normalized = text.to_owned();
    if let Some(run_dir) = run_dir {
        normalized = normalized.replace(run_dir, "<run-dir>");
    }
    normalized
        .split_whitespace()
        .map(|token| {
            if is_hex_address(token) {
                "<addr>"
            } else if token.chars().all(|character| character.is_ascii_digit()) {
                "<n>"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_hex_address(token: &str) -> bool {
    token.len() > 2
        && token.starts_with("0x")
        && token[2..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
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
        assert!(classify_memory_growth(&leaking, 0).unwrap().suspected);
        let small: Vec<_> = (0..20).map(|minute| sample(minute, 100 + minute)).collect();
        assert!(!classify_memory_growth(&small, 0).unwrap().suspected);
        let spike: Vec<_> = (0..20)
            .map(|minute| sample(minute, if minute == 10 { 500 } else { 100 }))
            .collect();
        assert!(!classify_memory_growth(&spike, 0).unwrap().suspected);
    }

    #[test]
    fn memory_classifier_discards_warmup_before_choosing_windows() {
        // A large startup spike followed by flat steady-state RSS is not a
        // leak once warmup is discarded, but looks like one without it.
        let mut samples: Vec<_> = (0..5).map(|minute| sample(minute, 400)).collect();
        let steady: Vec<_> = (5..25).map(|minute| sample(minute, 100)).collect();
        samples.extend(steady);
        let with_warmup = classify_memory_growth(&samples, 5).unwrap();
        assert!(!with_warmup.suspected);
        assert_eq!(with_warmup.baseline_bytes, 100 * 1024 * 1024);
        let without_warmup = classify_memory_growth(&samples, 0).unwrap();
        assert!(without_warmup.baseline_bytes > with_warmup.baseline_bytes);
    }

    #[test]
    fn theil_sen_downsampling_is_deterministic_and_preserves_slope_sign() {
        let rising: Vec<_> = (0..10_000)
            .map(|index| (index as f64, 100.0 + index as f64 * 2.0))
            .collect();
        let first = theil_sen_slope(&rising).unwrap();
        let second = theil_sen_slope(&rising).unwrap();
        assert_eq!(first, second);
        assert!(first > 0.0);
        let falling: Vec<_> = (0..10_000)
            .map(|index| (index as f64, 100.0 - index as f64))
            .collect();
        assert!(theil_sen_slope(&falling).unwrap() < 0.0);
        // Downsampling caps the estimator's pair count (O(n^2) guard).
        assert_eq!(
            super::downsample_points(&rising, THEIL_SEN_MAX_POINTS).len(),
            THEIL_SEN_MAX_POINTS
        );
    }

    #[test]
    fn exit_precedence_and_confirmation_gates_beat_hang_classes() {
        // An exited process must classify as exit/signal, never stall/hang.
        for (live, snapshot) in [
            (ProbeResult::ProcessExited, ProbeResult::TimedOut),
            (ProbeResult::TimedOut, ProbeResult::ProcessExited),
            (ProbeResult::ProcessExited, ProbeResult::ProcessExited),
        ] {
            assert_eq!(
                classify_hang(HangEvidence {
                    action_timed_out: true,
                    live,
                    snapshot,
                    consecutive_misses: 3,
                    process_progressed: false,
                }),
                None
            );
        }
        // Fewer than three consecutive misses is an ordinary command timeout.
        assert_eq!(
            classify_hang(HangEvidence {
                action_timed_out: true,
                live: ProbeResult::TimedOut,
                snapshot: ProbeResult::TimedOut,
                consecutive_misses: 2,
                process_progressed: false,
            }),
            None
        );
        // A progressing process is not a whole-process hang.
        assert_eq!(
            classify_hang(HangEvidence {
                action_timed_out: true,
                live: ProbeResult::TimedOut,
                snapshot: ProbeResult::TimedOut,
                consecutive_misses: 3,
                process_progressed: true,
            }),
            None
        );
    }

    #[test]
    fn log_gate_matches_panic_text_and_error_levels_precisely() {
        assert!(is_panic_or_error_log(
            "thread 'main' panicked at src/lib.rs:10:5:"
        ));
        assert!(is_panic_or_error_log("reached panic!() in dispatch"));
        assert!(is_panic_or_error_log("ERROR sotf: device open failed"));
        assert!(!is_panic_or_error_log("INFO errors=0 recovered"));
        assert!(!is_panic_or_error_log("no errors observed"));
    }

    #[test]
    fn normalize_signature_replaces_whole_hex_addresses_and_digits() {
        assert_eq!(
            normalize_signature(
                "panicked at 0x7fff5fbff8a8 after 42 iterations at 0x10",
                None
            ),
            "panicked at <addr> after <n> iterations at <addr>"
        );
        assert_eq!(
            normalize_signature("failure under /runs/abc dir", Some("/runs/abc")),
            "failure under <run-dir> dir"
        );
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
