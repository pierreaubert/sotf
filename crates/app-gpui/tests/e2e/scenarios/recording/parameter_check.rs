use crate::driver::AppDriver;
use crate::pages::recording::RecordingPage;
use crate::runner::E2ERunner;
use crate::runner::TestScenario;
use gpui::{VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::types::{
    CtcMatrixExportStrategy, RecordingSignalType, SpeakerConfiguration,
};
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

pub struct RecordingParameterCheckScenario;

impl TestScenario for RecordingParameterCheckScenario {
    fn name(&self) -> &'static str {
        "Recording Parameter Check"
    }

    fn setup(&mut self, _cx: &mut gpui::TestAppContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        view: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        let mut driver = AppDriver::new(cx, view);

        // 1. Navigate to Recording Page
        let mut page = RecordingPage::new(&mut driver);
        page.navigate_to_recording();

        // 2. Change Playback Device
        let test_device = "Buffered Output";
        page.set_playback_device(test_device);
        assert_eq!(page.get_playback_device(), test_device);

        // 3. Change Recording Device
        let test_mic = "Buffered Input";
        page.set_recording_device(test_mic);
        assert_eq!(page.get_recording_device(), test_mic);

        // 4. Change Sample Rate
        let test_rate = 96000;
        page.set_sample_rate(test_rate);
        assert_eq!(page.get_sample_rate(), test_rate);

        // 5. Change Speaker Configuration
        let test_config = SpeakerConfiguration::Stereo21;
        page.set_speaker_config(test_config);
        assert_eq!(page.get_speaker_config(), test_config);

        assert_eq!(page.get_channel_count(), 3);

        // 6. Change Signal Type
        let test_signal = RecordingSignalType::PinkNoise;
        page.set_signal_type(test_signal);
        assert_eq!(page.get_signal_type(), test_signal);

        // 7. Change Signal Duration
        let test_duration = 2.5;
        page.set_signal_duration(test_duration);
        assert!((page.get_signal_duration() - test_duration).abs() < 0.001);

        // 8. Change Sweep Range
        page.set_sweep_range(50.0, 15000.0);
        let (start, end) = page.get_sweep_range();
        assert!((start - 50.0).abs() < 0.001);
        assert!((end - 15000.0).abs() < 0.001);

        // 9. Change Signal Level
        let test_level = -12.5;
        page.set_signal_level(test_level);
        assert!((page.get_signal_level() - test_level).abs() < 0.001);

        // 10. Change Mic Calibration
        let cal_path = "/tmp/cal.txt";
        page.set_mic_calibration(cal_path);
        assert_eq!(page.get_mic_calibration(), Some(cal_path.to_string()));

        // 11. Change Recording Directory
        let rec_dir = "/tmp/recordings";
        page.set_recording_directory(rec_dir);
        assert_eq!(page.get_recording_directory(), Some(rec_dir.to_string()));

        // 12. Configure transfer-matrix capture
        assert_eq!(
            page.get_ctc_matrix_strategy(),
            CtcMatrixExportStrategy::ImpulseResponse
        );
        page.set_ctc_matrix_strategy(CtcMatrixExportStrategy::RawSweep);
        page.set_ctc_loopback_input(2);
        assert_eq!(
            page.get_ctc_matrix_strategy(),
            CtcMatrixExportStrategy::RawSweep
        );
        assert_eq!(page.get_ctc_loopback_input(), Some(2));

        Ok(())
    }

    fn teardown(&mut self, _cx: &mut gpui::TestAppContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

#[gpui::test]
async fn test_recording_parameters(cx: &mut gpui::TestAppContext) {
    let scenario = RecordingParameterCheckScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;

    if let Err(e) = &result {
        println!("Test failed: {}", e);
    }
    assert!(result.is_ok());
}
