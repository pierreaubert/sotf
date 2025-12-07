use autoeq_roomsim::{calculate_modal_pressure, Point3D};
use num_complex::Complex64;
use std::f64::consts::PI;

#[derive(Debug, Clone)]
pub struct MeasurementCurve {
    pub frequencies: Vec<f64>,
    pub magnitude_db: Vec<f64>,
    pub phase_deg: Option<Vec<f64>>, // Phase needed for complex addition/removal
}

#[derive(Debug, Clone)]
pub struct RoomMeasurement {
    pub curve: MeasurementCurve,
    /// Distance from source in meters (for reference)
    pub distance_m: f64,
    /// Absolute listener position in the room
    pub listener_position: Point3D,
}

#[derive(Debug, Clone)]
pub struct RoomCorrectionInput {
    pub measurements: Vec<RoomMeasurement>,
    /// Room dimensions [width, depth, height] in meters
    pub room_dimensions: [f64; 3],
    /// Source position in the room
    pub source_position: Point3D,
    /// Maximum frequency to apply room mode correction (Schroeder frequency)
    pub correction_limit_hz: f64,
    /// Speed of sound (m/s), default 343.0
    pub speed_of_sound: f64,
    /// Modal damping factor (Q), default 10.0
    pub modal_damping: f64,
    /// Max mode order to compute
    pub max_mode_order: u32,
}

impl Default for RoomCorrectionInput {
    fn default() -> Self {
        Self {
            measurements: Vec::new(),
            room_dimensions: [0.0; 3],
            source_position: Point3D { x: 0.0, y: 0.0, z: 0.0 },
            correction_limit_hz: 300.0,
            speed_of_sound: 343.0,
            modal_damping: 10.0,
            max_mode_order: 20,
        }
    }
}

/// Calculate the free-field response by removing room modes using multiple measurements
pub fn calculate_room_correction(input: &RoomCorrectionInput) -> Result<MeasurementCurve, String> {
    if input.measurements.is_empty() {
        return Err("No measurements provided".to_string());
    }

    // Validate frequency consistency
    let freqs = &input.measurements[0].curve.frequencies;
    let len = freqs.len();
    for m in &input.measurements {
        if m.curve.frequencies.len() != len {
            return Err("Measurement frequency counts mismatch".to_string());
        }
        // Ideally check values match too
    }

    let mut corrected_mag = Vec::with_capacity(len);
    let mut corrected_phase = Vec::with_capacity(len);
    
    // Compute modes once if possible? 
    // calculate_modal_pressure recomputes everything. 
    // For now we accept the inefficiency as the loop over frequencies is the outer loop here.
    
    for i in 0..len {
        let f = freqs[i];
        
        // Complex pressure measurements
        let mut measured_pressures = Vec::with_capacity(input.measurements.len());
        let mut modeled_tfs = Vec::with_capacity(input.measurements.len());

        for m in &input.measurements {
            let mag = m.curve.magnitude_db[i];
            
            let phase_deg = if let Some(p) = &m.curve.phase_deg {
                 p[i]
            } else {
                 return Err("Measurement missing phase data".to_string());
            };
            
            // Convert to linear pressure
            // P = 10^(dB/20) * e^(j * phase)
            let mag_lin = 10.0_f64.powf(mag / 20.0);
            let phase_rad = phase_deg.to_radians();
            let p_meas = Complex64::from_polar(mag_lin, phase_rad);
            measured_pressures.push(p_meas);

            // Compute modeled Transfer Function (simulated room response)
            if f <= input.correction_limit_hz {
                let h_sim = calculate_modal_pressure(
                    &input.source_position,
                    &m.listener_position,
                    f,
                    input.room_dimensions[0],
                    input.room_dimensions[1],
                    input.room_dimensions[2],
                    input.speed_of_sound,
                    input.max_mode_order,
                    input.modal_damping,
                );
                modeled_tfs.push(h_sim);
            } else {
                // Above limit: Use simple Direct field model (1/r * phase)
                let r = input.source_position.distance_to(&m.listener_position).max(0.01);
                let k = 2.0 * PI * f / input.speed_of_sound;
                let h_direct = Complex64::from_polar(1.0/r, -k*r);
                modeled_tfs.push(h_direct);
            }
        }

        // Solve for Source S using Least Squares:
        // M_k = S * H_k  => S = (sum M_k * H_k^*) / (sum |H_k|^2)
        
        let mut numerator = Complex64::new(0.0, 0.0);
        let mut denominator = 0.0;

        for (m_val, h_val) in measured_pressures.iter().zip(modeled_tfs.iter()) {
            numerator += m_val * h_val.conj();
            denominator += h_val.norm_sqr();
        }

        if denominator < 1e-12 {
            // Fallback for singularity
            corrected_mag.push(measured_pressures[0].norm());
            corrected_phase.push(measured_pressures[0].arg().to_degrees());
        } else {
            let s_est = numerator / denominator;
            
            // Convert back to dB/deg
            let s_db = 20.0 * s_est.norm().log10();
            let s_phase = s_est.arg().to_degrees();
            
            corrected_mag.push(s_db);
            corrected_phase.push(s_phase);
        }
    }

    Ok(MeasurementCurve {
        frequencies: freqs.clone(),
        magnitude_db: corrected_mag,
        phase_deg: Some(corrected_phase),
    })
}
