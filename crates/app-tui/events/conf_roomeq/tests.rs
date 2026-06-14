#[cfg(test)]
mod poll_tests {
    use std::sync::{Arc, Mutex};

    use crate::app::App;
    use crate::events::poll_room_eq_optimization;
    use crate::theme::Theme;
    use autoeq::roomeq::{PipelineStepId, PipelineStepStatus};
    use sotf_audio_player::autoeq::RoomOptimizationProgress;
    use sotf_audio_player::room_eq_types::OptimizationStatus;

    use super::super::consts::ROOM_OPT_PROGRESS;

    #[test]
    fn test_poll_room_eq_optimization_moves_strings_without_cloning() {
        let mut app = App::new(Theme::default(), false);
        app.room_eq.opt_status = OptimizationStatus::Running;

        let progress = RoomOptimizationProgress {
            current_speaker: "Left".to_string(),
            speaker_index: 0,
            total_speakers: 2,
            iteration: 50,
            max_iterations: 1000,
            loss: 0.5,
            overall_progress: 0.1,
            message: Some("Optimizing Left".to_string()),
            epa_preference: None,
            step_id: Some(PipelineStepId::GenericChannelOptimization),
            step_status: Some(PipelineStepStatus::InProgress),
        };

        let slot = ROOM_OPT_PROGRESS
            .get_or_init(|| Arc::new(Mutex::new(None)))
            .clone();
        *slot.lock().unwrap() = Some(progress);

        assert!(poll_room_eq_optimization(&mut app));
        assert_eq!(app.room_eq.opt_progress, 0.1f32);
        assert_eq!(app.room_eq.opt_current_speaker, "Left");
        assert_eq!(
            app.room_eq.opt_status_message.as_deref(),
            Some("Optimizing Left")
        );
        assert_eq!(
            app.room_eq.opt_log_lines.back().map(String::as_str),
            Some("Optimizing Left")
        );
        assert_eq!(app.room_eq.opt_iteration, 50);
    }
}
