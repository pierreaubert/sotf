use crate::driver::AppDriver;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::app::types::{
    CrossoverType, RoomEqAlgorithm, RoomEqDataSource, SpeakerConfigType,
};
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

pub struct RoomEqParameterCheckScenario;

impl TestScenario for RoomEqParameterCheckScenario {
    fn name(&self) -> &'static str {
        "Room EQ Parameter Check"
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

        // 1. Navigate to Room EQ (Setup)
        driver.update_app(|app, _| {
            // Need to set up some minimal state
            app.measurement_state.room_eq_state.speaker_configs.clear();
        });

        // 2. Change Data Source
        driver.update_app(|app, _| {
            app.measurement_state.room_eq_state.data_source =
                RoomEqDataSource::FromFile("/tmp/test.json".into());
        });
        driver.read_app(|app| {
            if let RoomEqDataSource::FromFile(path) =
                &app.measurement_state.room_eq_state.data_source
            {
                assert_eq!(path.to_str().unwrap(), "/tmp/test.json");
            } else {
                panic!("Expected FromFile data source");
            }
        });

        // 3. Configure Optimizer
        driver.update_app(|app, _| {
            app.measurement_state
                .room_eq_state
                .optimizer_config
                .algorithm = RoomEqAlgorithm::NelderMead;
            app.measurement_state
                .room_eq_state
                .optimizer_config
                .num_filters = 8;
            app.measurement_state.room_eq_state.optimizer_config.min_q = 0.8;
            app.measurement_state.room_eq_state.optimizer_config.max_q = 4.0;
            app.measurement_state.room_eq_state.optimizer_config.min_db = -10.0;
            app.measurement_state.room_eq_state.optimizer_config.max_db = 8.0;
            app.measurement_state
                .room_eq_state
                .optimizer_config
                .min_freq = 40.0;
            app.measurement_state
                .room_eq_state
                .optimizer_config
                .max_freq = 18000.0;
            app.measurement_state
                .room_eq_state
                .optimizer_config
                .max_iter = 5000;
        });

        driver.read_app(|app| {
            let config = &app.measurement_state.room_eq_state.optimizer_config;
            assert_eq!(config.algorithm, RoomEqAlgorithm::NelderMead);
            assert_eq!(config.num_filters, 8);
            assert!((config.min_q - 0.8).abs() < 0.001);
            assert!((config.max_q - 4.0).abs() < 0.001);
            assert!((config.min_db - -10.0).abs() < 0.001);
            assert!((config.max_db - 8.0).abs() < 0.001);
            assert!((config.min_freq - 40.0).abs() < 0.001);
            assert!((config.max_freq - 18000.0).abs() < 0.001);
            assert_eq!(config.max_iter, 5000);
        });

        // 4. Configure Speaker (Add one first)
        driver.update_app(|app, _| {
            use sotf_audio_player_gpui::app::types::RoomEqSpeakerConfig;
            app.measurement_state
                .room_eq_state
                .speaker_configs
                .push(RoomEqSpeakerConfig {
                    channel_name: "L".to_string(),
                    config_type: SpeakerConfigType::Single, // Start as single
                    crossover_type: CrossoverType::LR24,
                    driver_names: vec![],
                    crossover_freq_hints: vec![],
                    cardioid_separation_m: None,
                });
        });

        // Change speaker params
        driver.update_app(|app, _| {
            let speaker = &mut app.measurement_state.room_eq_state.speaker_configs[0];
            speaker.config_type = SpeakerConfigType::MultiDriver;
            speaker.crossover_type = CrossoverType::Butterworth24;
            speaker.driver_names = vec!["W".to_string(), "T".to_string()];
            speaker.crossover_freq_hints = vec![2500.0];
        });

        driver.read_app(|app| {
            let speaker = &app.measurement_state.room_eq_state.speaker_configs[0];
            assert_eq!(speaker.config_type, SpeakerConfigType::MultiDriver);
            assert_eq!(speaker.crossover_type, CrossoverType::Butterworth24);
            assert_eq!(speaker.driver_names.len(), 2);
            assert_eq!(speaker.driver_names[0], "W");
            assert_eq!(speaker.crossover_freq_hints[0], 2500.0);
        });

        Ok(())
    }

    fn teardown(&mut self, _cx: &mut gpui::TestAppContext) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

#[gpui::test]
async fn test_room_eq_parameters(cx: &mut gpui::TestAppContext) {
    let scenario = RoomEqParameterCheckScenario;
    let runner = E2ERunner::new(scenario);
    let result = runner.run(cx).await;

    if let Err(e) = &result {
        println!("Test failed: {}", e);
    }
    assert!(result.is_ok());
}
