//! Room mode calculations

use num_complex::Complex64;
use std::f64::consts::PI;

// Re-export types from parent
pub use crate::{Point3D, RoomMode};

/// Calculate room modes for a rectangular room
pub fn calculate_room_modes(
    length_x: f64,
    length_y: f64,
    length_z: f64,
    speed_of_sound: f64,
    max_frequency: f64,
    max_order: u32,
) -> Vec<RoomMode> {
    let mut modes = Vec::new();
    let c = speed_of_sound;

    for n in 0..=max_order {
        for m in 0..=max_order {
            for p in 0..=max_order {
                if n == 0 && m == 0 && p == 0 {
                    continue;
                }

                let nx = n as f64 / length_x;
                let my = m as f64 / length_y;
                let pz = p as f64 / length_z;
                let freq = (c / 2.0) * (nx * nx + my * my + pz * pz).sqrt();

                if freq > max_frequency {
                    continue;
                }

                let zero_count = [n, m, p].iter().filter(|&&x| x == 0).count();
                let mode_type = match zero_count {
                    2 => "axial",
                    1 => "tangential",
                    0 => "oblique",
                    _ => continue,
                };

                let description = match (n, m, p) {
                    (n, 0, 0) if n > 0 => format!("{},0,0 - Length mode (X)", n),
                    (0, m, 0) if m > 0 => format!("0,{},0 - Width mode (Y)", m),
                    (0, 0, p) if p > 0 => format!("0,0,{} - Height mode (Z)", p),
                    (n, m, 0) => format!("{},{},0 - Floor tangential", n, m),
                    (n, 0, p) => format!("{},0,{} - Side tangential", n, p),
                    (0, m, p) => format!("0,{},{} - Front tangential", m, p),
                    (n, m, p) => format!("{},{},{} - Oblique", n, m, p),
                };

                modes.push(RoomMode {
                    frequency: freq,
                    indices: [n, m, p],
                    mode_type: mode_type.to_string(),
                    description,
                });
            }
        }
    }

    modes.sort_by(|a, b| a.frequency.partial_cmp(&b.frequency).unwrap());
    modes
}

/// Calculate modal pressure at a point
#[allow(clippy::too_many_arguments)]
pub fn calculate_modal_pressure(
    source: &Point3D,
    listener: &Point3D,
    frequency: f64,
    room_width: f64,
    room_depth: f64,
    room_height: f64,
    speed_of_sound: f64,
    max_mode_order: u32,
    modal_damping: f64,
) -> Complex64 {
    let volume = room_width * room_depth * room_height;
    let r = source.distance_to(listener).max(0.1);
    let omega = 2.0 * PI * frequency;
    let omega_sq = omega * omega;
    let k = omega / speed_of_sound;
    let c_sq = speed_of_sound * speed_of_sound;
    let prefactor = c_sq / volume;
    let mut modal_sum = Complex64::new(0.0, 0.0);

    for n in 0..=max_mode_order {
        for m in 0..=max_mode_order {
            for p in 0..=max_mode_order {
                if n == 0 && m == 0 && p == 0 {
                    continue;
                }

                let nx = n as f64 / room_width;
                let my = m as f64 / room_depth;
                let pz = p as f64 / room_height;
                let omega_n = speed_of_sound * PI * (nx * nx + my * my + pz * pz).sqrt();
                let omega_n_sq = omega_n * omega_n;
                let mode_freq = omega_n / (2.0 * PI);

                if mode_freq > frequency * 4.0 || mode_freq < frequency / 4.0 {
                    continue;
                }

                let source_mode = (n as f64 * PI * source.x / room_width).cos()
                    * (m as f64 * PI * source.y / room_depth).cos()
                    * (p as f64 * PI * source.z / room_height).cos();

                let listener_mode = (n as f64 * PI * listener.x / room_width).cos()
                    * (m as f64 * PI * listener.y / room_depth).cos()
                    * (p as f64 * PI * listener.z / room_height).cos();

                let epsilon = |i: u32| if i == 0 { 1.0 } else { 2.0 };
                let mode_norm = epsilon(n) * epsilon(m) * epsilon(p);

                let delta_n = omega_n / (2.0 * modal_damping);
                let denominator = Complex64::new(omega_n_sq - omega_sq, -2.0 * delta_n * omega);
                let transfer_function = Complex64::new(1.0, 0.0) / denominator;

                let mode_amplitude = mode_norm * source_mode * listener_mode;
                modal_sum += transfer_function * mode_amplitude;
            }
        }
    }

    modal_sum *= prefactor;
    let phase = Complex64::new(0.0, k * r).exp();
    modal_sum * phase
}

/// Hybrid crossover weight
pub fn hybrid_crossover_weight(
    frequency: f64,
    schroeder_frequency: f64,
    crossover_width_octaves: f64,
) -> f64 {
    if schroeder_frequency <= 0.0 {
        return 1.0;
    }

    let octaves_from_schroeder = (frequency / schroeder_frequency).log2();
    let half_width = crossover_width_octaves / 2.0;

    if octaves_from_schroeder < -half_width {
        0.0
    } else if octaves_from_schroeder > half_width {
        1.0
    } else {
        let t = (octaves_from_schroeder + half_width) / crossover_width_octaves;
        (1.0 - (t * PI).cos()) / 2.0
    }
}
