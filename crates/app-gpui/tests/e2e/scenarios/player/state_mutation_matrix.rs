use crate::driver::AppDriver;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::Screen;
use sotf_audio_player_gpui::app::types::{
    OptimizationStatus, RecordingStep, RoomEqStep, ToastMessage,
};
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

struct StateMutationMatrixScenario;

impl TestScenario for StateMutationMatrixScenario {
    fn name(&self) -> &'static str {
        "GPUI focused loading and error state mutation matrix"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, window);

        driver.update_app(|app, _| {
            app.library_view.loading_initial_data = true;
        });
        for screen in [Screen::Home, Screen::HomeShelf, Screen::Library] {
            driver.navigate_to(screen);
            assert_eq!(driver.read_app(|app| app.ui_state.current_screen), screen);
        }
        driver.update_app(|app, _| {
            app.library_view.loading_initial_data = false;
            app.ui_state.toast_message = Some(ToastMessage::error("Fixture library error"));
        });
        driver.navigate_to(Screen::Library);
        assert!(driver.read_app(|app| app.ui_state.toast_message.is_some()));

        driver.update_app(|app, _| {
            let headphone = &mut app.measurement_state.headphone_eq_state;
            headphone.loading_headphones = true;
            headphone.error_message = None;
        });
        driver.navigate_to(Screen::HeadphoneEq);
        driver.update_app(|app, _| {
            let headphone = &mut app.measurement_state.headphone_eq_state;
            headphone.loading_headphones = false;
            headphone.error_message = Some("Fixture headphone error".to_string());
        });
        driver.run_until_parked();

        driver.update_app(|app, _| {
            let spinorama = &mut app.measurement_state.spinorama_eq_state;
            spinorama.loading_speakers = true;
            spinorama.error_message = None;
        });
        driver.navigate_to(Screen::Spinorama);
        driver.update_app(|app, _| {
            let spinorama = &mut app.measurement_state.spinorama_eq_state;
            spinorama.loading_speakers = false;
            spinorama.error_message = Some("Fixture Spinorama error".to_string());
        });
        driver.run_until_parked();

        driver.update_app(|app, _| {
            let room_eq = &mut app.measurement_state.room_eq_state;
            room_eq.step = RoomEqStep::Optimize;
            room_eq.optimization_status = OptimizationStatus::Running;
            room_eq.status_message = "Fixture optimization running".to_string();
            room_eq.error_message = None;
        });
        driver.navigate_to(Screen::RoomEq);
        driver.update_app(|app, _| {
            let room_eq = &mut app.measurement_state.room_eq_state;
            room_eq.optimization_status = OptimizationStatus::Failed;
            room_eq.error_message = Some("Fixture RoomEQ error".to_string());
        });
        driver.run_until_parked();

        driver.update_app(|app, _| {
            let recording = &mut app.measurement_state.recording_state;
            recording.step = RecordingStep::Capture;
            recording.status_message = "Fixture recording status".to_string();
        });
        driver.navigate_to(Screen::Recording);

        Ok(())
    }
}

#[gpui::test]
async fn focused_loading_and_error_states_render_after_mutation(cx: &mut TestAppContext) {
    E2ERunner::new(StateMutationMatrixScenario)
        .run(cx)
        .await
        .unwrap();
}
