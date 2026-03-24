//! Room acoustics calculations

pub use crate::RoomAcoustics;

/// Calculate RT60 using Sabine's formula
pub fn rt60_sabine(volume: f64, total_absorption: f64) -> f64 {
    if total_absorption > 0.0 {
        0.161 * volume / total_absorption
    } else {
        f64::INFINITY
    }
}

/// Calculate RT60 using Eyring's formula
pub fn rt60_eyring(volume: f64, surface_area: f64, average_alpha: f64) -> f64 {
    if average_alpha > 0.0 && average_alpha < 1.0 {
        let ln_term = (1.0 - average_alpha).ln();
        if ln_term != 0.0 {
            0.161 * volume / (-surface_area * ln_term)
        } else {
            f64::INFINITY
        }
    } else {
        f64::INFINITY
    }
}

/// Calculate critical distance
pub fn critical_distance(volume: f64, rt60: f64) -> f64 {
    if rt60 > 0.0 {
        (volume / (std::f64::consts::PI * rt60)).sqrt()
    } else {
        0.0
    }
}

/// Calculate room acoustics parameters
pub fn calculate_room_acoustics(
    volume: f64,
    surface_area: f64,
    total_absorption: f64,
    _speed_of_sound: f64,
) -> RoomAcoustics {
    let average_alpha = if surface_area > 0.0 {
        total_absorption / surface_area
    } else {
        0.0
    };

    let rt60_sabine_val = rt60_sabine(volume, total_absorption);
    let rt60_eyring_val = rt60_eyring(volume, surface_area, average_alpha);

    let rt60 = if average_alpha > 0.1 {
        rt60_eyring_val
    } else {
        rt60_sabine_val
    };

    let schroeder_frequency = if rt60 > 0.0 && volume > 0.0 {
        2000.0 / (volume * rt60).sqrt()
    } else {
        500.0
    };

    let critical_distance_val = critical_distance(volume, rt60);

    RoomAcoustics {
        rt60_sabine: rt60_sabine_val,
        rt60_eyring: rt60_eyring_val,
        volume,
        surface_area,
        average_alpha,
        total_absorption,
        schroeder_frequency,
        critical_distance: critical_distance_val,
    }
}
