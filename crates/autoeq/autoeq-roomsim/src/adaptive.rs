//! Adaptive resolution and performance optimization
//!
//! Provides utilities for dynamic quality adjustment based on
//! device capabilities and computation time.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityMode {
    Preview,
    Medium,
    High,
    Ultra,
}

impl QualityMode {
    pub fn slice_resolution(&self) -> (u32, u32) {
        match self {
            QualityMode::Preview => (16, 16),
            QualityMode::Medium => (32, 32),
            QualityMode::High => (64, 64),
            QualityMode::Ultra => (128, 128),
        }
    }

    pub fn num_frequency_points(&self) -> usize {
        match self {
            QualityMode::Preview => 32,
            QualityMode::Medium => 64,
            QualityMode::High => 128,
            QualityMode::Ultra => 256,
        }
    }

    pub fn chunk_size(&self) -> usize {
        match self {
            QualityMode::Preview => 8,
            QualityMode::Medium => 16,
            QualityMode::High => 32,
            QualityMode::Ultra => 64,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveConfig {
    pub quality_mode: QualityMode,
    pub auto_adjust: bool,
    pub target_frame_time_ms: f64,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            quality_mode: QualityMode::Medium,
            auto_adjust: true,
            target_frame_time_ms: 16.67, // ~60fps
        }
    }
}

#[wasm_bindgen]
pub struct AdaptiveState {
    quality_mode: QualityMode,
    auto_adjust: bool,
    target_frame_time_ms: f64,
    consecutive_slow_frames: u32,
    consecutive_fast_frames: u32,
    current_slice_resolution: u32,
}

#[wasm_bindgen]
impl AdaptiveState {
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: &str) -> Self {
        let config: AdaptiveConfig = if config_json.is_empty() {
            AdaptiveConfig::default()
        } else {
            serde_json::from_str(config_json).unwrap_or_default()
        };

        let (res, _) = config.quality_mode.slice_resolution();

        Self {
            quality_mode: config.quality_mode,
            auto_adjust: config.auto_adjust,
            target_frame_time_ms: config.target_frame_time_ms,
            consecutive_slow_frames: 0,
            consecutive_fast_frames: 0,
            current_slice_resolution: res,
        }
    }

    pub fn get_slice_resolution(&self) -> u32 {
        self.current_slice_resolution
    }

    pub fn get_chunk_size(&self) -> usize {
        self.quality_mode.chunk_size()
    }

    pub fn get_quality_mode(&self) -> String {
        match self.quality_mode {
            QualityMode::Preview => "preview".to_string(),
            QualityMode::Medium => "medium".to_string(),
            QualityMode::High => "high".to_string(),
            QualityMode::Ultra => "ultra".to_string(),
        }
    }

    #[wasm_bindgen]
    pub fn report_frame_time(&mut self, frame_time_ms: f64) {
        if !self.auto_adjust {
            return;
        }

        if frame_time_ms > self.target_frame_time_ms * 1.5 {
            // Frame took too long
            self.consecutive_slow_frames += 1;
            self.consecutive_fast_frames = 0;

            if self.consecutive_slow_frames >= 3 {
                self.decrease_quality();
                self.consecutive_slow_frames = 0;
            }
        } else if frame_time_ms < self.target_frame_time_ms * 0.7 {
            // Frame was fast
            self.consecutive_fast_frames += 1;
            self.consecutive_slow_frames = 0;

            if self.consecutive_fast_frames >= 10 {
                self.increase_quality();
                self.consecutive_fast_frames = 0;
            }
        }
    }

    fn decrease_quality(&mut self) {
        self.quality_mode = match self.quality_mode {
            QualityMode::Ultra => QualityMode::High,
            QualityMode::High => QualityMode::Medium,
            QualityMode::Medium => QualityMode::Preview,
            QualityMode::Preview => QualityMode::Preview,
        };
        let (res, _) = self.quality_mode.slice_resolution();
        self.current_slice_resolution = res;
    }

    fn increase_quality(&mut self) {
        self.quality_mode = match self.quality_mode {
            QualityMode::Preview => QualityMode::Medium,
            QualityMode::Medium => QualityMode::High,
            QualityMode::High => QualityMode::Ultra,
            QualityMode::Ultra => QualityMode::Ultra,
        };
        let (res, _) = self.quality_mode.slice_resolution();
        self.current_slice_resolution = res;
    }

    pub fn set_quality_mode(&mut self, mode: &str) {
        self.quality_mode = match mode {
            "preview" => QualityMode::Preview,
            "medium" => QualityMode::Medium,
            "high" => QualityMode::High,
            "ultra" => QualityMode::Ultra,
            _ => return,
        };
        let (res, _) = self.quality_mode.slice_resolution();
        self.current_slice_resolution = res;
    }

    pub fn to_json(&self) -> String {
        let result = serde_json::json!({
            "quality_mode": self.get_quality_mode(),
            "slice_resolution": self.current_slice_resolution,
            "chunk_size": self.get_chunk_size(),
            "auto_adjust": self.auto_adjust,
            "target_frame_time_ms": self.target_frame_time_ms,
        });
        result.to_string()
    }
}

/// Detect optimal quality mode based on device
#[wasm_bindgen]
pub fn detect_optimal_quality() -> String {
    let threads = crate::worker_detect::num_threads_available();

    let mode = if threads >= 8 {
        QualityMode::Ultra
    } else if threads >= 4 {
        QualityMode::High
    } else if threads >= 2 {
        QualityMode::Medium
    } else {
        QualityMode::Preview
    };

    mode.to_string()
}

impl std::fmt::Display for QualityMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QualityMode::Preview => f.write_str("preview"),
            QualityMode::Medium => f.write_str("medium"),
            QualityMode::High => f.write_str("high"),
            QualityMode::Ultra => f.write_str("ultra"),
        }
    }
}

impl Default for AdaptiveState {
    fn default() -> Self {
        Self::new("")
    }
}
