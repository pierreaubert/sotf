use sotf_host::detector::DetectionMode;

pub(super) const MAX_BLOCK_FRAMES: usize = 4096;

pub(super) const MAX_LOOKAHEAD_MS: f32 = 20.0;

pub(super) fn parse_detection_mode(s: &str) -> Result<DetectionMode, String> {
    match s.to_ascii_lowercase().as_str() {
        "rms" => Ok(DetectionMode::Rms { window_ms: 10.0 }),
        "peak" => Ok(DetectionMode::Peak),
        _ => Err(format!("unknown detection mode: {s}")),
    }
}
