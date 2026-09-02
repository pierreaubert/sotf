//! Pure helpers for dev-API performance metrics.

use std::time::Duration;

use anyhow::{Result, anyhow};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct AllocationSnapshot {
    pub(crate) count: u64,
    pub(crate) requested_bytes: u64,
}

pub(crate) fn allocation_snapshot_from_value(value: &Value) -> Result<AllocationSnapshot> {
    let requested_bytes = statistic_total(value, "malloc_requested")?;
    let count = [
        "malloc_normal_count",
        "malloc_huge_count",
        "malloc_guarded_count",
    ]
    .into_iter()
    .try_fold(0_u64, |total, name| {
        statistic_total(value, name).map(|value| total.saturating_add(value))
    })?;
    Ok(AllocationSnapshot {
        count,
        requested_bytes,
    })
}

fn statistic_total(value: &Value, name: &str) -> Result<u64> {
    let statistic = value
        .get(name)
        .ok_or_else(|| anyhow!("mimalloc statistic `{name}` is missing"))?;
    statistic
        .as_u64()
        .or_else(|| statistic.get("total").and_then(Value::as_u64))
        .ok_or_else(|| anyhow!("mimalloc statistic `{name}` has no unsigned total"))
}

pub(crate) fn percentile_ms(values: &mut [Duration], percentile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_unstable();
    let rank = ((values.len() as f64) * percentile).ceil() as usize;
    duration_ms(values[rank.saturating_sub(1).min(values.len() - 1)])
}

pub(crate) fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

pub(crate) fn rate(value: u64, elapsed_seconds: f64) -> f64 {
    if elapsed_seconds > 0.0 {
        value as f64 / elapsed_seconds
    } else {
        0.0
    }
}

pub(crate) fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator > 0 {
        numerator as f64 / denominator as f64
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn percentile_uses_nearest_rank() {
        let mut values = [
            Duration::from_millis(4),
            Duration::from_millis(1),
            Duration::from_millis(3),
            Duration::from_millis(2),
        ];
        assert_eq!(percentile_ms(&mut values, 0.50), 2.0);
        assert_eq!(percentile_ms(&mut values, 0.95), 4.0);
    }

    #[test]
    fn parses_mimalloc_counter_and_count_shapes() {
        let value = json!({
            "malloc_requested": { "total": 4096, "peak": 100, "current": 50 },
            "malloc_normal_count": 9,
            "malloc_huge_count": 2,
            "malloc_guarded_count": 1
        });
        assert_eq!(
            allocation_snapshot_from_value(&value).unwrap(),
            AllocationSnapshot {
                count: 12,
                requested_bytes: 4096,
            }
        );
    }

    #[cfg(feature = "dev-api")]
    #[test]
    fn installed_mimalloc_exposes_expected_statistics() {
        let stats = mimalloc::MiMalloc::stats_json().unwrap();
        let value: Value = serde_json::from_slice(stats.to_bytes()).unwrap();
        allocation_snapshot_from_value(&value).unwrap();
    }
}
