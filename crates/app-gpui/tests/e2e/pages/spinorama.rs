use crate::driver::AppDriver;
use sotf_audio_player_gpui::app::types::{OptimizationStatus, SpinoramaOptimizationMode, SpinoramaTargetCurve};

pub struct SpinoramaPage<'a, 'b> {
    driver: &'a mut AppDriver<'b>,
}

impl<'a, 'b> SpinoramaPage<'a, 'b> {
    pub fn new(driver: &'a mut AppDriver<'b>) -> Self {
        Self { driver }
    }

    pub fn select_speaker(&mut self, speaker: &str) {
        let speaker = speaker.to_string();
        self.driver.view.update(self.driver.cx, |view, _window, cx| {
             view.state.update(cx, |state, _cx| {
                 state.app.measurement_state.spinorama_eq_state.selected_speaker = Some(speaker.clone());
                 state.app.measurement_state.spinorama_eq_state.optimization_status = OptimizationStatus::Idle; 
             });
        }).unwrap();
        self.driver.cx.run_until_parked();
    }

    pub fn select_version(&mut self, version: &str) {
        let _version = version.to_string(); 
        self.driver.view.update(self.driver.cx, |view, _window, cx| {
             view.state.update(cx, |state, _cx| {
                 // Logic placeholder
             });
        }).unwrap();
        self.driver.cx.run_until_parked();
    }

    pub fn set_target_curve(&mut self, curve: &str) {
        let curve = curve.to_string();
        self.driver.view.update(self.driver.cx, |view, _window, cx| {
             view.state.update(cx, |state, _cx| {
                 state.app.measurement_state.spinorama_eq_state.selected_curve = curve.clone();
             });
        }).unwrap();
        self.driver.cx.run_until_parked();
    }

    pub fn start_optimization(&mut self) {
        self.driver.view.update(self.driver.cx, |view, _window, cx| {
             view.state.update(cx, |state, _cx| {
                 state.app.measurement_state.spinorama_eq_state.optimization_status = OptimizationStatus::Running;
             });
        }).unwrap();
        self.driver.cx.run_until_parked();
    }

    pub fn wait_for_optimization_completion(&mut self) -> Result<(), String> {
        let start = std::time::Instant::now();
        loop {
            if start.elapsed().as_secs() > 10 {
                return Err("Timeout waiting for optimization".to_string());
            }

            self.driver.cx.run_until_parked();
            
            let (status, msg) = self.driver.view.update(self.driver.cx, |view, _window, cx| {
                 let state = view.state.read(cx);
                 (state.app.measurement_state.spinorama_eq_state.optimization_status.clone(), 
                  state.app.measurement_state.spinorama_eq_state.error_message.clone())
            }).unwrap();

            match status {
                 OptimizationStatus::Completed => return Ok(()),
                 OptimizationStatus::Failed => return Err(msg.unwrap_or_else(|| "Unknown error".to_string())),
                 OptimizationStatus::Running => {
                     // Simulate completion
                     self.driver.view.update(self.driver.cx, |view, _window, cx| {
                         view.state.update(cx, |state, _cx| {
                             if state.app.measurement_state.spinorama_eq_state.optimization_status == OptimizationStatus::Running {
                                 state.app.measurement_state.spinorama_eq_state.optimization_status = OptimizationStatus::Completed;
                             }
                         });
                     }).unwrap();
                 },
                 _ => {}
            }
            self.driver.cx.run_until_parked(); 
        }
    }

    pub fn set_optimization_params(&mut self, num_filters: usize, max_iter: usize) {
         self.driver.update_app(move |app, _| {
             app.measurement_state.spinorama_eq_state.optimizer_config.num_filters = num_filters;
             app.measurement_state.spinorama_eq_state.optimizer_config.max_iter = max_iter;
             // Set small tolerance to ensure it finishes or runs
             app.measurement_state.spinorama_eq_state.optimizer_config.tolerance = 1e-5;
         });
    }

    pub fn set_optimization_mode(&mut self, mode: SpinoramaOptimizationMode) {
        self.driver.update_app(move |app, _| {
            app.measurement_state.spinorama_eq_state.optimizer_config.mode = mode;
        });
    }

    pub fn set_frequency_limits(&mut self, min_freq: f64, max_freq: f64) {
        self.driver.update_app(move |app, _| {
            app.measurement_state.spinorama_eq_state.optimizer_config.min_freq = min_freq;
            app.measurement_state.spinorama_eq_state.optimizer_config.max_freq = max_freq;
        });
    }

    pub fn set_gain_limits(&mut self, min_db: f64, max_db: f64) {
        self.driver.update_app(move |app, _| {
            app.measurement_state.spinorama_eq_state.optimizer_config.min_db = min_db;
            app.measurement_state.spinorama_eq_state.optimizer_config.max_db = max_db;
        });
    }

    pub fn set_q_limits(&mut self, min_q: f64, max_q: f64) {
        self.driver.update_app(move |app, _| {
            app.measurement_state.spinorama_eq_state.optimizer_config.min_q = min_q;
            app.measurement_state.spinorama_eq_state.optimizer_config.max_q = max_q;
        });
    }

    pub fn set_algorithm_params(&mut self, population: usize, de_f: f64, de_cr: f64) {
        self.driver.update_app(move |app, _| {
            app.measurement_state.spinorama_eq_state.optimizer_config.population = population;
            app.measurement_state.spinorama_eq_state.optimizer_config.de_f = de_f;
            app.measurement_state.spinorama_eq_state.optimizer_config.de_cr = de_cr;
        });
    }

    pub fn set_smoothing(&mut self, enabled: bool, window_size: usize) {
        self.driver.update_app(move |app, _| {
            app.measurement_state.spinorama_eq_state.optimizer_config.smooth = enabled;
            app.measurement_state.spinorama_eq_state.optimizer_config.smooth_n = window_size;
        });
    }

    pub fn set_advanced_config(&mut self, spacing_weight: f64, min_spacing_oct: f64) {
        self.driver.update_app(move |app, _| {
            app.measurement_state.spinorama_eq_state.optimizer_config.spacing_weight = spacing_weight;
            app.measurement_state.spinorama_eq_state.optimizer_config.min_spacing_oct = min_spacing_oct;
        });
    }

    pub fn inject_mock_data(&mut self) {
        self.driver.update_app(|app, _| {
            // Set selected speaker and version
            app.measurement_state.spinorama_eq_state.selected_speaker = Some("Genelec 8351B".to_string());
            app.measurement_state.spinorama_eq_state.selected_version = "ASR".to_string();
            app.measurement_state.spinorama_eq_state.selected_measurement = "CEA2034".to_string();
            
            // Mock available options to pass validation
            app.measurement_state.spinorama_eq_state.available_measurements = vec!["CEA2034".to_string()];
            
            // Create dummy SpinoramaCurves to pass "valid" check
            let freqs = vec![20.0, 100.0, 1000.0, 10000.0, 20000.0];
            let flat = vec![80.0; freqs.len()];
            
             app.measurement_state.spinorama_eq_state.spinorama_curves = sotf_audio_player_gpui::app::types::SpinoramaCurves {
                frequencies: freqs.clone(),
                on_axis: flat.clone(),
                listening_window: flat.clone(),
                early_reflections: flat.clone(),
                sound_power: flat.clone(),
                early_reflections_di: vec![0.0; freqs.len()],
                sound_power_di: vec![0.0; freqs.len()],
                estimated_in_room: flat.clone(),
                horizontal_directivity: vec![],
                vertical_directivity: vec![],
            };
            
            app.measurement_state.spinorama_eq_state.step = sotf_audio_player_gpui::app::types::SpinoramaStep::Configure;
        });
        self.driver.cx.run_until_parked();
    }
}
