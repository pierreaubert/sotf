#[cfg(test)]
pub(crate) fn spinorama_step_prev(s: crate::app::SpinoramaStep) -> crate::app::SpinoramaStep {
    use crate::app::SpinoramaStep;
    match s {
        SpinoramaStep::Select => SpinoramaStep::Select, // no wrap
        SpinoramaStep::Configure => SpinoramaStep::Select,
        SpinoramaStep::Optimize => SpinoramaStep::Configure,
        SpinoramaStep::Results => SpinoramaStep::Optimize,
        SpinoramaStep::UpdatePlugin => SpinoramaStep::Results,
    }
}

#[cfg(test)]
pub(crate) fn spinorama_step_next(s: crate::app::SpinoramaStep) -> crate::app::SpinoramaStep {
    use crate::app::SpinoramaStep;
    match s {
        SpinoramaStep::Select => SpinoramaStep::Configure,
        SpinoramaStep::Configure => SpinoramaStep::Optimize,
        SpinoramaStep::Optimize => SpinoramaStep::Results,
        SpinoramaStep::Results => SpinoramaStep::UpdatePlugin,
        SpinoramaStep::UpdatePlugin => SpinoramaStep::UpdatePlugin, // no wrap
    }
}

use crate::app::SpinoramaStep;

#[test]
fn spinorama_step_prev_does_not_wrap() {
    assert_eq!(
        spinorama_step_prev(SpinoramaStep::Select),
        SpinoramaStep::Select,
    );
}

#[test]
fn spinorama_step_next_does_not_wrap() {
    assert_eq!(
        spinorama_step_next(SpinoramaStep::UpdatePlugin),
        SpinoramaStep::UpdatePlugin,
    );
}

#[test]
fn spinorama_step_round_trip() {
    let steps = [
        SpinoramaStep::Select,
        SpinoramaStep::Configure,
        SpinoramaStep::Optimize,
        SpinoramaStep::Results,
        SpinoramaStep::UpdatePlugin,
    ];
    for i in 0..steps.len() - 1 {
        assert_eq!(spinorama_step_next(steps[i]), steps[i + 1]);
        assert_eq!(spinorama_step_prev(steps[i + 1]), steps[i]);
    }
}

#[cfg(test)]
mod poll_tests {
    use std::sync::{Arc, Mutex};

    use crate::app::App;
    use crate::events::poll_spinorama_optimization;
    use crate::theme::Theme;
    use sotf_audio_player::autoeq::SpeakerOptimizationResult;
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    use super::super::consts::OPT_RESULT;

    fn make_result() -> SpeakerOptimizationResult {
        SpeakerOptimizationResult {
            biquads: vec![math_audio_iir_fir::Biquad::new(
                math_audio_iir_fir::BiquadFilterType::Lowshelf,
                80.0,
                48000.0,
                0.7,
                2.0,
            )],
            frequencies: Vec::new(),
            input_curve: Vec::new(),
            target_curve: Vec::new(),
            deviation_curve: Vec::new(),
            filter_response: Vec::new(),
            error_curve: Vec::new(),
            corrected_curve: Vec::new(),
            normalized_curve: Vec::new(),
            individual_filter_responses: Vec::new(),
            output_path: String::new(),
            on_axis_curve: Vec::new(),
            lw_curve: Vec::new(),
            er_curve: Vec::new(),
            sp_curve: Vec::new(),
            pir_curve: Vec::new(),
            er_di_curve: Vec::new(),
            sp_di_curve: Vec::new(),
            optimization_history: Vec::new(),
            initial_loss: 1.0,
            final_loss: 0.1,
            crossover_freqs: None,
            driver_gains: None,
            driver_delays: None,
        }
    }

    #[test]
    fn test_poll_spinorama_optimization_maps_filter_type_to_long_name() {
        let mut app = App::new(Theme::default(), false);
        app.spinorama_eq.model.optimization_status = OptimizationStatus::Running;

        let slot = OPT_RESULT
            .get_or_init(|| Arc::new(Mutex::new(None)))
            .clone();
        *slot.lock().unwrap() = Some(Ok(make_result()));

        assert!(poll_spinorama_optimization(&mut app));
        assert_eq!(app.spinorama_eq.model.filters.len(), 1);
        assert_eq!(app.spinorama_eq.model.filters[0].filter_type, "Lowshelf");
        assert_eq!(
            app.spinorama_eq.model.optimization_status,
            OptimizationStatus::Completed
        );
    }
}
