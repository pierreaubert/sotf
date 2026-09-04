use crate::params::CROSSOVER_TYPES;
use serde::{Deserialize, Serialize};
use sotf_host::define_choice_string_deserializer;

define_choice_string_deserializer!(deserialize_crossover_type, CROSSOVER_TYPES);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossoverPluginParams {
    #[serde(rename = "type", deserialize_with = "deserialize_crossover_type")]
    pub crossover_type: String,
    pub frequency: f64,
    pub output: String,
    /// Additional crossover frequencies for 3-way or 4-way mode.
    /// When provided, creates a multi-way crossover. The primary `frequency`
    /// becomes the first crossover point.
    #[serde(default)]
    pub extra_frequencies: Vec<f64>,
    /// FIR taps for linear-phase crossover mode. Even values are rounded up.
    #[serde(default)]
    pub fir_taps: Option<usize>,
    /// Per-channel crossover frequencies in Hz. When non-empty, switches the
    /// plugin into per-channel mode (one independent LR24 crossover per
    /// channel) and the scalar `frequency` / `output` fields are ignored.
    #[serde(default)]
    pub channel_frequencies_hz: Vec<f32>,
    /// Per-channel mode for each channel in per-channel mode: "lowpass",
    /// "highpass", "mute" (channel outputs silence), or "passthrough". Must match
    /// `channel_frequencies_hz.len()` when both are non-empty.
    #[serde(default)]
    pub channel_modes: Vec<String>,
}
