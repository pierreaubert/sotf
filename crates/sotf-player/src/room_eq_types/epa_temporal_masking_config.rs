use super::default::default_epa_temporal_enabled;
use super::default::default_epa_temporal_ir_enabled;
use super::default::default_epa_temporal_ir_weight;
use super::default::default_epa_temporal_post_mask_ms;
use super::default::default_epa_temporal_pre_mask_ms;
use super::default::default_epa_temporal_weight;
use super::epa_temporal_profile::EpaTemporalProfile;
use serde::{Deserialize, Serialize};

/// UI-facing surface for EPA temporal-masking knobs.
///
/// Maps onto [`autoeq::loss::epa::score::TemporalMaskingConfig`] one-to-one;
/// kept separate so the UI never has to import the backend type directly and
/// so additional UI-only state (e.g. expanded/collapsed) can sit next to the
/// data without bleeding into the JSON contract with autoeq.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpaTemporalMaskingConfig {
    /// Master toggle: when false the optimizer skips both the modal and the
    /// IR temporal-masking penalties (the rest of EPA still runs).
    #[serde(default = "default_epa_temporal_enabled")]
    pub enabled: bool,
    /// Weight for the modal (frequency-domain) temporal-masking penalty.
    #[serde(default = "default_epa_temporal_weight")]
    pub weight: f64,
    /// Material profile that scales pre/post ringing audibility.
    #[serde(default)]
    pub profile: EpaTemporalProfile,
    /// Enable the direct FIR impulse-response pre/post-ringing analysis.
    /// Only meaningful when FIR coefficients are exported.
    #[serde(default = "default_epa_temporal_ir_enabled")]
    pub ir_enabled: bool,
    /// Weight for the FIR IR-masking penalty term.
    #[serde(default = "default_epa_temporal_ir_weight")]
    pub ir_weight: f64,
    /// Pre-masking window in ms (energy inside the window is partially
    /// masked; outside is fully audible).
    #[serde(default = "default_epa_temporal_pre_mask_ms")]
    pub pre_mask_ms: f64,
    /// Post-masking window in ms.
    #[serde(default = "default_epa_temporal_post_mask_ms")]
    pub post_mask_ms: f64,
}

impl Default for EpaTemporalMaskingConfig {
    fn default() -> Self {
        Self {
            enabled: default_epa_temporal_enabled(),
            weight: default_epa_temporal_weight(),
            profile: EpaTemporalProfile::default(),
            ir_enabled: default_epa_temporal_ir_enabled(),
            ir_weight: default_epa_temporal_ir_weight(),
            pre_mask_ms: default_epa_temporal_pre_mask_ms(),
            post_mask_ms: default_epa_temporal_post_mask_ms(),
        }
    }
}

impl EpaTemporalMaskingConfig {
    /// Returns true when the user has knobs set away from the autoeq defaults
    /// — used by `to_optimizer_config` to decide whether to override the
    /// backend's `epa_config` at all.
    pub fn differs_from_default(&self) -> bool {
        let d = Self::default();
        self.enabled != d.enabled
            || (self.weight - d.weight).abs() > f64::EPSILON
            || self.profile != d.profile
            || self.ir_enabled != d.ir_enabled
            || (self.ir_weight - d.ir_weight).abs() > f64::EPSILON
            || (self.pre_mask_ms - d.pre_mask_ms).abs() > f64::EPSILON
            || (self.post_mask_ms - d.post_mask_ms).abs() > f64::EPSILON
    }

    /// Build a room-model `TemporalMaskingConfig`, leaving non-UI knobs at the
    /// autoeq defaults. Spread-init keeps any future backend fields at their
    /// `Default::default()` without forcing this layer to track them.
    pub fn to_backend(&self) -> autoeq::roomeq_model::TemporalMaskingConfig {
        autoeq::roomeq_model::TemporalMaskingConfig {
            enabled: self.enabled,
            weight: self.weight,
            profile: match self.profile {
                EpaTemporalProfile::Transient => {
                    autoeq::roomeq_model::TemporalMaskingProfile::Transient
                }
                EpaTemporalProfile::Mixed => autoeq::roomeq_model::TemporalMaskingProfile::Mixed,
                EpaTemporalProfile::Sustained => {
                    autoeq::roomeq_model::TemporalMaskingProfile::Sustained
                }
            },
            ir_enabled: self.ir_enabled,
            ir_weight: self.ir_weight,
            pre_mask_ms: self.pre_mask_ms,
            post_mask_ms: self.post_mask_ms,
            ..autoeq::roomeq_model::TemporalMaskingConfig::default()
        }
    }
}
