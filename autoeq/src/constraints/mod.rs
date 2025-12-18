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

/// Ceiling constraint to limit maximum EQ boost.
pub mod ceiling;
/// Constraint for crossover frequency monotonicity in multi-driver systems.
pub mod crossover_monotonicity;
/// Minimum gain constraint for EQ filters.
pub mod min_gain;
/// Minimum frequency spacing constraint between adjacent filters.
pub mod min_spacing;

pub use ceiling::*;
pub use crossover_monotonicity::*;
pub use min_gain::*;
pub use min_spacing::*;
