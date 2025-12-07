use crate::optimization_params::OptimizationParams;
use crate::ui::PlayerView;
use gpui::prelude::*;
use gpui::*;


/// Result of a speaker optimization run
#[derive(Clone, Debug)]
pub struct SpeakerOptimizationResult {
    pub biquads: Vec<autoeq_iir::Biquad>,
    pub frequencies: Vec<f64>,
    pub input_curve: Vec<f64>,        // On-axis or listening window
    pub target_curve: Vec<f64>,       // Calculated target
    pub deviation_curve: Vec<f64>,    // Input - Target
    pub filter_response: Vec<f64>,    // Sum of biquads
    pub error_curve: Vec<f64>,        // Deviation + Filter
    pub corrected_curve: Vec<f64>,    // Input + Filter
    pub individual_filter_responses: Vec<Vec<f64>>,
    pub output_path: String,
    
    // Spinorama specific curves
    pub er_curve: Vec<f64>,           // Early Reflections
    pub sp_curve: Vec<f64>,           // Sound Power
    pub er_di_curve: Vec<f64>,        // Early Reflections Directivity Index
    pub sp_di_curve: Vec<f64>,        // Sound Power Directivity Index
    
    pub optimization_history: Vec<(usize, f64)>,
    pub initial_loss: f64,
    pub final_loss: f64,
}

impl PlayerView {
    /// Run speaker optimization
    pub fn run_speaker_optimization(&mut self, cx: &mut Context<Self>) {
        let (speaker_model, params, export_format) = {
            let state = self.state.read(cx);
            if state.app.speaker_model.is_empty() {
                self.state.update(cx, |state, _cx| {
                    state.app.toast_message = Some(crate::app::ToastMessage::error(
                        "Please select a speaker model",
                    ));
                });
                cx.notify();
                return;
            }
            (
                state.app.speaker_model.clone(),
                state.app.speaker_params.clone(),
                state.app.speaker_export_format.clone(),
            )
        };

        self.state.update(cx, |state, _cx| {
            state.app.speaker_optimization_running = true;
            state.app.speaker_optimization_progress.clear();
            state.app.speaker_optimization_result = None;
        });
        cx.notify();
        
        // Create a callback for progress updates
        // Note: For now we'll just simulate progress or handle it if we have a real backend
        // Since run_speaker_optimization_task is async, we spawn it.

        cx.spawn(async move |view: WeakEntity<PlayerView>, mut cx| {
            let result = run_speaker_optimization_task(
                speaker_model,
                String::new(), // Target not used yet/handled internally
                String::new(),
                params,
                export_format,
            ).await;

            view.update(cx, |view, cx| {
                view.state.update(cx, |state, _cx| {
                    state.app.speaker_optimization_running = false;
                    match result {
                        Ok(res) => {
                            state.app.speaker_optimization_result = Some(res);
                            state.app.toast_message = Some(crate::app::ToastMessage::success(
                                "Speaker optimization completed",
                            ));
                        }
                        Err(e) => {
                             state.app.toast_message = Some(crate::app::ToastMessage::error(format!(
                                "Optimization failed: {}", e
                            )));
                        }
                    }
                });
                cx.notify();
            }).ok();
        }).detach();
    }
}

/// Run speaker optimization in a background task
pub async fn run_speaker_optimization_task(
    speaker_model: String,
    _target: String,
    _target_custom_path: String,
    params: OptimizationParams,
    _export_format: String,
) -> Result<SpeakerOptimizationResult, String> {
    // Simulate delay for dummy task
    if speaker_model == "Dummy Speaker" {
         smol::Timer::after(std::time::Duration::from_millis(500)).await;
         return Ok(generate_dummy_result(params));
    }

    Err("Downloading speaker data is not yet implemented. Please select 'Dummy Speaker' for UI testing.".to_string())
}

fn generate_dummy_result(_params: OptimizationParams) -> SpeakerOptimizationResult {
    let n = 200;
    let frequencies: Vec<f64> = (0..n).map(|i| 20.0 * (1000.0f64).powf(i as f64 / n as f64)).collect();
    let input_curve: Vec<f64> = frequencies.iter().map(|f| (f/1000.0).sin() * 5.0).collect();
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
        individual_filter_responses: Vec::new(),
        output_path: "/tmp/speaker_eq.txt".to_string(),
        er_curve: input_curve.iter().map(|v| v - 3.0).collect(),
        sp_curve: input_curve.iter().map(|v| v - 5.0).collect(),
        er_di_curve: vec![3.0; n],
        sp_di_curve: vec![5.0; n],
        optimization_history: vec![(0, 1.0), (10, 0.5), (20, 0.1)],
        initial_loss: 1.0,
        final_loss: 0.1,
    }
}
