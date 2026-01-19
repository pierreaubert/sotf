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

mod filter_color;
mod plot_drivers;
mod plot_filters;
mod plot_results;
mod plot_spin;
mod ref_lines;
mod trend_lines;

pub use filter_color::filter_color;
pub use plot_drivers::{plot_drivers, plot_drivers_results};
pub use plot_filters::plot_filters;
pub use plot_results::plot_results;
pub use plot_spin::{plot_spin, plot_spin_details, plot_spin_tonal};
pub use trend_lines::*;
