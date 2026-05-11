//! AutoEQ - A library for audio equalization and filter optimization
//! Loss functions and types for AutoEQ optimizer
//!
//! Copyright (C) 2025-2026 Pierre Aubert pierre(at)spinorama(dot)org
//!
//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//! GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program.  If not, see <https://www.gnu.org/licenses/>.

pub mod asymmetric;
pub mod bass_boost;
pub mod drivers;
pub mod enhanced_weights;
pub mod epa;
pub mod flat;
pub mod headphone;
pub mod multisub;
pub mod phase_aware;
pub mod slope;
pub mod speaker;
pub mod types;

pub use asymmetric::{AsymmetricLossConfig, flat_loss_asymmetric, weighted_mse_asymmetric};
pub use drivers::{
    CrossoverType, DriverMeasurement, DriversLossData, compute_drivers_combined_response,
    compute_drivers_combined_response_complex, compute_per_driver_responses, drivers_flat_loss,
};
pub use flat::flat_loss;
pub use headphone::{headphone_loss, headphone_loss_with_target};
pub use multisub::multisub_flat_loss;
pub use slope::{
    calculate_standard_deviation_in_range, curve_slope_per_octave_in_range,
    regression_slope_per_octave_in_range,
};
pub use speaker::{mixed_loss, speaker_score_loss};
pub use types::{HeadphoneLossData, LossType, SpeakerLossData};
