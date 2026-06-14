pub use autoeq::SpeakerOptResult;

/// Result of a speaker optimization run
#[derive(Clone, Debug)]
pub struct SpeakerOptimizationResult {
    pub biquads: Vec<math_audio_iir_fir::Biquad>,
    pub frequencies: Vec<f64>,
    pub input_curve: Vec<f64>,
    pub target_curve: Vec<f64>,
    pub deviation_curve: Vec<f64>,
    pub filter_response: Vec<f64>,
    pub error_curve: Vec<f64>,
    pub corrected_curve: Vec<f64>,
    pub normalized_curve: Vec<f64>,
    pub individual_filter_responses: Vec<Vec<f64>>,
    pub output_path: String,

    // Spinorama specific curves (from CEA2034 data)
    pub on_axis_curve: Vec<f64>,
    pub lw_curve: Vec<f64>,
    pub er_curve: Vec<f64>,
    pub sp_curve: Vec<f64>,
    pub pir_curve: Vec<f64>,
    pub er_di_curve: Vec<f64>,
    pub sp_di_curve: Vec<f64>,

    pub optimization_history: Vec<(usize, f64)>,
    pub initial_loss: f64,
    pub final_loss: f64,

    // Multi-driver results (optional)
    pub crossover_freqs: Option<Vec<f64>>,
    pub driver_gains: Option<Vec<f64>>,
    pub driver_delays: Option<Vec<f64>>,
}

impl From<SpeakerOptResult> for SpeakerOptimizationResult {
    fn from(result: SpeakerOptResult) -> Self {
        // Extract spin data curves if available.
        //
        // When spin data is absent (headphone mode, or speakers without a
        // CEA2034 measurement), the seven spinorama curves are returned as
        // empty `Vec<f64>` — NOT zero-filled vectors. Downstream renderers
        // (e.g. `speaker_graphs::render_spinorama_main_response_plot`) rely
        // on `is_empty()` to detect absent data and pick a fallback curve;
        // a zero-filled vector silently passes that check and produces a
        // misleading flat-line plot at 0 dB.
        let (on_axis, lw, er, sp, pir, er_di, sp_di) = if let Some(ref spin) = result.spin_data {
            (
                spin.on_axis.spl.iter().copied().collect(),
                spin.listening_window.spl.iter().copied().collect(),
                spin.early_reflections.spl.iter().copied().collect(),
                spin.sound_power.spl.iter().copied().collect(),
                spin.estimated_in_room.spl.iter().copied().collect(),
                spin.er_di.spl.iter().copied().collect(),
                spin.sp_di.spl.iter().copied().collect(),
            )
        } else {
            (
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        };

        Self {
            biquads: result.biquads,
            frequencies: result.curves.frequencies,
            input_curve: result.curves.input_curve.clone(),
            target_curve: result.curves.target_curve,
            deviation_curve: result.curves.deviation_curve,
            filter_response: result.curves.filter_response,
            error_curve: result.curves.error_curve,
            corrected_curve: result.curves.corrected_curve,
            normalized_curve: result.curves.input_curve.clone(),
            individual_filter_responses: result.curves.individual_filter_responses,
            output_path: String::new(),
            on_axis_curve: on_axis,
            lw_curve: lw,
            er_curve: er,
            sp_curve: sp,
            pir_curve: pir,
            er_di_curve: er_di,
            sp_di_curve: sp_di,
            optimization_history: result.history,
            initial_loss: result.initial_loss,
            final_loss: result.final_loss,
            crossover_freqs: None,
            driver_gains: None,
            driver_delays: None,
        }
    }
}

pub(super) fn generate_dummy_result() -> SpeakerOptimizationResult {
    let n = 200;
    let frequencies: Vec<f64> = (0..n)
        .map(|i| 20.0 * (1000.0f64).powf(i as f64 / n as f64))
        .collect();
    let input_curve: Vec<f64> = frequencies
        .iter()
        .map(|f| (f / 1000.0).sin() * 5.0)
        .collect();
    let target_curve: Vec<f64> = vec![0.0; n];

    SpeakerOptimizationResult {
        biquads: Vec::new(),
        frequencies: frequencies.clone(),
        input_curve: input_curve.clone(),
        target_curve: target_curve.clone(),
        deviation_curve: input_curve.clone(),
        filter_response: vec![0.0; n],
        error_curve: input_curve.clone(),
        corrected_curve: input_curve.clone(),
        normalized_curve: input_curve.clone(),
        individual_filter_responses: Vec::new(),
        output_path: "/tmp/speaker_eq.txt".to_string(),
        on_axis_curve: input_curve.clone(),
        lw_curve: input_curve.clone(),
        er_curve: input_curve.iter().map(|v| v - 3.0).collect(),
        sp_curve: input_curve.iter().map(|v| v - 5.0).collect(),
        pir_curve: input_curve.iter().map(|v| v - 2.0).collect(),
        er_di_curve: vec![3.0; n],
        sp_di_curve: vec![5.0; n],
        optimization_history: vec![(0, 1.0), (10, 0.5), (20, 0.1)],
        initial_loss: 1.0,
        final_loss: 0.1,
        crossover_freqs: None,
        driver_gains: None,
        driver_delays: None,
    }
}
