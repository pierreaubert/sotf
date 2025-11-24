//! AutoEQ - A library for audio equalization and filter optimization
//!
//! Copyright (C) 2025 Pierre Aubert pierre(at)spinorama(dot)org
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

mod clamp;
mod directory;
mod interpolate;
mod normalize;
mod plot;
mod read_api;
mod read_csv;
mod smooth;
mod speaker_suggestions;

// Re-export commonly used functions
pub use clamp::clamp_positive_only;
pub use directory::data_dir_for;
pub use directory::measurement_filename;
pub use directory::sanitize_dir_name;
pub use interpolate::*;
pub use normalize::*;
pub use read_api::*;
pub use read_csv::load_driver_measurement;
pub use read_csv::load_frequency_response;
pub use read_csv::read_curve_from_csv;
pub use smooth::smooth_gaussian;
pub use smooth::smooth_one_over_n_octave;
pub use speaker_suggestions::*;
