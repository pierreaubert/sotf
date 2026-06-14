/// Recursively flatten a JSON value into dotted key-value pairs.
/// Skips large arrays (e.g. measurement data) — only includes scalars and small objects.
/// `true` when a pipeline step status implies the step is the
/// optimizer's current focus. Started/InProgress events update
/// `current_step`; Completed/Skipped events only land in
/// `step_history` so the previous step doesn't keep claiming
/// "current" once the next step has taken over.
pub(super) fn is_active_step(status: sotf_audio_player::autoeq::PipelineStepStatus) -> bool {
    use sotf_audio_player::autoeq::PipelineStepStatus;
    matches!(
        status,
        PipelineStepStatus::Started | PipelineStepStatus::InProgress
    )
}

/// Finalize the pipeline-step indicators when an optimization run
/// reaches a terminal state. Clears `current_step` (so no chip stays
/// in "active" colour) and, on success, promotes any in-flight
/// (`Started`/`InProgress`) entries in `step_history` to `Completed`
/// so the strip reads as a fully-green summary instead of leaving
/// "what was running when the run ended" half-coloured.
pub(super) fn finalize_pipeline_step_state(
    room_eq: &mut crate::app::types::RoomEqState,
    succeeded: bool,
) {
    use sotf_audio_player::autoeq::PipelineStepStatus;
    room_eq.current_step = None;
    if succeeded {
        for status in room_eq.step_history.values_mut() {
            if matches!(
                status,
                PipelineStepStatus::Started | PipelineStepStatus::InProgress
            ) {
                *status = PipelineStepStatus::Completed;
            }
        }
    }
}

pub(super) fn flatten_json(
    value: &serde_json::Value,
    prefix: String,
    pairs: &mut Vec<(String, String)>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                flatten_json(v, key, pairs);
            }
        }
        serde_json::Value::Array(arr) => {
            // Skip large arrays (measurement data, etc.)
            if arr.len() <= 8 {
                for (i, v) in arr.iter().enumerate() {
                    let key = format!("{}[{}]", prefix, i);
                    flatten_json(v, key, pairs);
                }
            }
        }
        serde_json::Value::String(s) => {
            pairs.push((prefix, s.clone()));
        }
        serde_json::Value::Number(n) => {
            pairs.push((prefix, n.to_string()));
        }
        serde_json::Value::Bool(b) => {
            pairs.push((prefix, b.to_string()));
        }
        serde_json::Value::Null => {
            pairs.push((prefix, "null".to_string()));
        }
    }
}

pub(super) fn downsample_xy(x: &[f64], y: &[f64], max_points: usize) -> (Vec<f64>, Vec<f64>) {
    if max_points == 0 || x.len() <= max_points || y.len() <= max_points {
        return (x.to_vec(), y.to_vec());
    }

    let len = x.len().min(y.len());
    if len <= max_points {
        return (x[..len].to_vec(), y[..len].to_vec());
    }

    let last = len - 1;
    let denom = max_points - 1;
    let mut xs = Vec::with_capacity(max_points);
    let mut ys = Vec::with_capacity(max_points);
    let mut previous = usize::MAX;
    for i in 0..max_points {
        let idx = (i * last + denom / 2) / denom;
        if idx != previous {
            xs.push(x[idx]);
            ys.push(y[idx]);
            previous = idx;
        }
    }
    (xs, ys)
}
