use crate::autoeq::speaker_eq::SpeakerOptimizationResult;

#[derive(Debug, Clone)]
pub struct MeasurementCurve {
    pub frequencies: Vec<f64>,
    pub magnitude_db: Vec<f64>,
    pub phase_deg: Option<Vec<f64>>, // Optional phase
}

#[derive(Debug, Clone)]
pub struct NearfieldMeasurement {
    pub curve: MeasurementCurve,
    pub driver_radius_mm: f64, // For piston scaling if needed
    pub enclosure_diffraction_db: Option<Vec<f64>>, // Baffle step correction
}

#[derive(Debug, Clone)]
pub struct FarfieldMeasurement {
    pub curve: MeasurementCurve,
    pub gate_start_ms: f64,
    pub gate_end_ms: f64,
    pub mic_distance_m: f64,
}

/// Helper to merge Nearfield and Farfield measurements into a single quasi-anechoic response
pub fn merge_nearfield_farfield(
    nearfields: &[NearfieldMeasurement],
    farfield: &FarfieldMeasurement,
    splice_freq_hz: f64,
) -> Result<MeasurementCurve, String> {
    // 0. Validate inputs (freq counts, etc.)
    
    // 1. Sum nearfield sources (Woofer + Port) if complex sum required?
    // usually sum complex pressures.
    
    // 2. Apply Baffle Step Diffraction to summed nearfield.
    
    // 3. Level match Nearfield to Farfield at merge freq.
    
    // 4. Splice.
    
    // Dummy implementation for now: return farfield (or check logic)
    // We need real interpolation and splicing logic.
    
    // For scaffolding, returning a clone of farfield with note.
    Ok(farfield.curve.clone()) 
}

/// Input data for calculating full Spinorama
pub struct SpinoramaInput {
    pub on_axis: MeasurementCurve,
    pub horizontal_measurements: Vec<(f64, MeasurementCurve)>, // angle -> curve
    pub vertical_measurements: Vec<(f64, MeasurementCurve)>, // angle -> curve
}

pub struct SpinoramaCalculator;

impl SpinoramaCalculator {
    // Calculate power average of multiple curves (dB values)
    fn power_average(curves: &[&MeasurementCurve]) -> Result<MeasurementCurve, String> {
        if curves.is_empty() { return Err("No curves to average".to_string()); }
        
        let len = curves[0].frequencies.len();
        let freqs = curves[0].frequencies.clone();
        
        // Ensure all curves have compatible frequency points
        for c in curves {
            if c.frequencies.len() != len {
                 return Err("Curve frequency length mismatch".to_string());
            }
            // Ideally check individual frequency values match
        }
        
        // Sum of powers: sum(10^(dB/10))
        let mut sum_power = vec![0.0; len];
        
        for c in curves {
            for (i, &db) in c.magnitude_db.iter().enumerate() {
                sum_power[i] += 10f64.powf(db / 10.0);
            }
        }
        
        let count = curves.len() as f64;
        let avg_mag: Vec<f64> = sum_power.iter().map(|&p| 10.0 * (p / count).log10()).collect();
        
        Ok(MeasurementCurve {
            frequencies: freqs,
            magnitude_db: avg_mag,
            phase_deg: None, // Average phase is tricky, skipping for stats
        })
    }
    
    // Helper to find a specific angle in measurements (tolerance 1.0 deg)
    fn find_curve<'a>(measurements: &'a [(f64, MeasurementCurve)], angle_deg: f64) -> Option<&'a MeasurementCurve> {
        measurements.iter()
            .find(|(a, _)| (a - angle_deg).abs() < 1.0)
            .map(|(_, c)| c)
    }

    pub fn calculate(input: &SpinoramaInput) -> Result<SpeakerOptimizationResult, String> {
        // --- 1. Listening Window Calculation ---
        // LW = Average(OnAxis, H+/-10, H+/-20, H+/-30, V+/-10)
        let mut lw_curves = Vec::new();
        lw_curves.push(&input.on_axis);
        
        // Horizontal +/- 10, 20, 30
        for &angle in &[10.0, -10.0, 20.0, -20.0, 30.0, -30.0] {
             if let Some(c) = Self::find_curve(&input.horizontal_measurements, angle) {
                 lw_curves.push(c);
             }
        }
        // Vertical +/- 10
        for &angle in &[10.0, -10.0] {
             if let Some(c) = Self::find_curve(&input.vertical_measurements, angle) {
                 lw_curves.push(c);
             }
        }
        
        let listening_window = Self::power_average(&lw_curves).map_err(|e| format!("LW calc error: {}", e))?;
        
        // --- 2. Early Reflections ---
        // Simplified ER average (example)
        // Only Front Wall, Side, Rear?
        // Standard defines weighted average of many angles.
        // For scaffold, we assume missing angles are OK or just use what we have.
        // ... (placeholder) ...
        let er_curve = listening_window.magnitude_db.clone(); // Placeholder
        let sp_curve = listening_window.magnitude_db.iter().map(|x| x - 5.0).collect(); // Placeholder
        
        // --- 3. Directivity Index ---
        // DI = Listening Window - Sound Power
        // (Or OnAxis - Sound Power for SPDI? Standard distinguishes SPDI and ERDI)
        // ERDI = Listening Window - Early Reflections
        // SPDI = Listening Window - Sound Power
        
        // ...
        
        // Result construction
        // Use input.on_axis for frequencies
        let n = input.on_axis.frequencies.len();
        
        Ok(SpeakerOptimizationResult {
            biquads: Vec::new(),
            frequencies: input.on_axis.frequencies.clone(),
            input_curve: input.on_axis.magnitude_db.clone(), // Show On-Axis as input
            target_curve: vec![0.0; n],
            deviation_curve: vec![0.0; n],
            filter_response: vec![0.0; n],
            error_curve: vec![0.0; n],
            corrected_curve: vec![0.0; n],
            individual_filter_responses: Vec::new(),
            output_path: String::new(),
            er_curve,
            sp_curve,
            er_di_curve: vec![0.0; n], // Placeholder
            sp_di_curve: vec![0.0; n], // Placeholder
            optimization_history: Vec::new(),
            initial_loss: 0.0,
            final_loss: 0.0,
        })
    }
}
