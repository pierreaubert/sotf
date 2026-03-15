use crate::driver::AppDriver;
use sotf_audio_player_gpui::app::types::{HeadphoneEqStep, OptimizationStatus};

pub struct HeadphoneEqPage<'a, 'b> {
    driver: &'a mut AppDriver<'b>,
}

impl<'a, 'b> HeadphoneEqPage<'a, 'b> {
    pub fn new(driver: &'a mut AppDriver<'b>) -> Self {
        Self { driver }
    }

    pub fn inject_mock_data(&mut self) {
        self.driver.update_app(|app, _| {
            app.measurement_state.headphone_eq_state.measurement_path =
                Some("/tmp/test_headphone.csv".to_string());
            app.measurement_state.headphone_eq_state.loss_type = "score".to_string();
            app.measurement_state.headphone_eq_state.target_preset =
                "harman-over-ear-2018".to_string();
            app.measurement_state.headphone_eq_state.step = HeadphoneEqStep::Optimization;
        });
        self.driver.cx.run_until_parked();
    }

    pub fn select_headphone(&mut self, _name: &str) {
        self.driver.update_app(|app, _| {
            app.measurement_state.headphone_eq_state.measurement_path =
                Some("/tmp/test_headphone.csv".to_string());
        });
        self.driver.cx.run_until_parked();
    }

    pub fn set_target_curve(&mut self, curve: &str) {
        let curve = curve.to_string();
        self.driver.update_app(move |app, _| {
            app.measurement_state.headphone_eq_state.target_preset = curve;
        });
        self.driver.cx.run_until_parked();
    }

    pub fn set_loss_type(&mut self, loss_type: &str) {
        let loss_type = loss_type.to_string();
        self.driver.update_app(move |app, _| {
            app.measurement_state
                .headphone_eq_state
                .set_ui_loss_type(&loss_type);
        });
        self.driver.cx.run_until_parked();
    }

    pub fn set_optimization_params(&mut self, num_filters: usize, max_iter: usize) {
        self.driver.update_app(move |app, _| {
            app.measurement_state
                .headphone_eq_state
                .optimizer_config
                .num_filters = num_filters;
            app.measurement_state
                .headphone_eq_state
                .optimizer_config
                .max_iter = max_iter;
            app.measurement_state
                .headphone_eq_state
                .optimizer_config
                .tolerance = 1e-5;
        });
    }

    pub fn set_frequency_limits(&mut self, min_freq: f64, max_freq: f64) {
        self.driver.update_app(move |app, _| {
            app.measurement_state
                .headphone_eq_state
                .optimizer_config
                .min_freq = min_freq;
            app.measurement_state
                .headphone_eq_state
                .optimizer_config
                .max_freq = max_freq;
        });
    }

    pub fn set_gain_limits(&mut self, min_db: f64, max_db: f64) {
        self.driver.update_app(move |app, _| {
            app.measurement_state
                .headphone_eq_state
                .optimizer_config
                .min_db = min_db;
            app.measurement_state
                .headphone_eq_state
                .optimizer_config
                .max_db = max_db;
        });
    }

    pub fn set_q_limits(&mut self, min_q: f64, max_q: f64) {
        self.driver.update_app(move |app, _| {
            app.measurement_state
                .headphone_eq_state
                .optimizer_config
                .min_q = min_q;
            app.measurement_state
                .headphone_eq_state
                .optimizer_config
                .max_q = max_q;
        });
    }

    pub fn start_optimization(&mut self) {
        self.driver.update_app(|app, _| {
            app.measurement_state.headphone_eq_state.optimization_status =
                OptimizationStatus::Running;
        });
        self.driver.cx.run_until_parked();
    }

    pub fn wait_for_optimization_completion(&mut self) -> Result<(), String> {
        let start = std::time::Instant::now();
        loop {
            if start.elapsed().as_secs() > 10 {
                return Err("Timeout waiting for optimization".to_string());
            }

            self.driver.cx.run_until_parked();

            let (status, msg) = self
                .driver
                .view
                .update(self.driver.cx, |view, _window, cx| {
                    let state = view.state.read(cx);
                    (
                        state
                            .app
                            .measurement_state
                            .headphone_eq_state
                            .optimization_status,
                        state
                            .app
                            .measurement_state
                            .headphone_eq_state
                            .error_message
                            .clone(),
                    )
                })
                .unwrap();

            match status {
                OptimizationStatus::Completed => return Ok(()),
                OptimizationStatus::Failed => {
                    return Err(msg.unwrap_or_else(|| "Unknown error".to_string()));
                }
                OptimizationStatus::Running => {
                    self.driver
                        .view
                        .update(self.driver.cx, |view, _window, cx| {
                            view.state.update(cx, |state, _cx| {
                                if state
                                    .app
                                    .measurement_state
                                    .headphone_eq_state
                                    .optimization_status
                                    == OptimizationStatus::Running
                                {
                                    state
                                        .app
                                        .measurement_state
                                        .headphone_eq_state
                                        .optimization_status = OptimizationStatus::Completed;
                                }
                            });
                        })
                        .unwrap();
                }
                _ => {}
            }
            self.driver.cx.run_until_parked();
        }
    }

    pub fn get_current_step(&mut self) -> HeadphoneEqStep {
        self.driver
            .read_app(|app| app.measurement_state.headphone_eq_state.step)
    }

    pub fn get_optimization_status(&mut self) -> OptimizationStatus {
        self.driver
            .read_app(|app| app.measurement_state.headphone_eq_state.optimization_status)
    }

    pub fn has_result(&mut self) -> bool {
        self.driver
            .read_app(|app| app.measurement_state.headphone_eq_state.result.is_some())
    }
}
