use super::driver_manager::DriverManager;
use super::pipeline_reconfigure_outcome::handle_driver_config_change;
use super::pipeline_spec::PipelineSpec;
use super::pipeline_supervisor::PipelineSupervisor;
use super::types::PipelinePlan;
use parking_lot::Mutex;
use sotf_audio::PluginConfig;
use sotf_audio::engine::PluginGraphConfig;
use sotf_audio::manager::AudioEngineManager;
use std::sync::Arc;

#[derive(Debug, Default)]
pub(super) struct SystemwideState {
    pub(super) pipeline: PipelineSupervisor,
}

impl SystemwideState {
    pub(super) fn selected_output_device(&self) -> Option<String> {
        self.pipeline.selected_output_device()
    }

    pub(super) fn user_plugins(&self) -> Vec<PluginConfig> {
        self.pipeline.user_plugins()
    }

    pub(super) fn user_graph(&self) -> Option<PluginGraphConfig> {
        self.pipeline.user_graph()
    }

    pub(super) fn input_channels(&self) -> usize {
        self.pipeline.input_channels()
    }

    pub(super) fn output_channels(&self) -> usize {
        self.pipeline.output_channels()
    }

    pub(super) fn input_loudness_index(&self) -> Option<usize> {
        self.pipeline.input_loudness_index()
    }

    pub(super) fn output_loudness_index(&self) -> Option<usize> {
        self.pipeline.output_loudness_index()
    }

    pub(super) fn applied_generation(&self) -> Option<u64> {
        self.pipeline.applied_generation()
    }

    pub(super) fn applied_output_device(&self) -> Option<String> {
        self.pipeline.applied_output_device()
    }

    pub(super) fn desired_spec(&self) -> PipelineSpec {
        self.pipeline.desired_spec()
    }

    pub(super) fn applied_spec(&self) -> Option<PipelineSpec> {
        self.pipeline.applied_spec()
    }

    pub(super) fn prepare_plan(
        &self,
        user_plugins: Vec<PluginConfig>,
        input_channels: usize,
        output_channels: usize,
        driver_input_fallback_channels: usize,
    ) -> Result<PipelinePlan, String> {
        self.pipeline.prepare_plan(
            user_plugins,
            input_channels,
            output_channels,
            driver_input_fallback_channels,
        )
    }

    pub(super) fn prepare_graph_plan(
        &self,
        user_graph: PluginGraphConfig,
        input_channels: usize,
        output_channels: usize,
        driver_input_fallback_channels: usize,
    ) -> Result<PipelinePlan, String> {
        self.pipeline.prepare_graph_plan(
            user_graph,
            input_channels,
            output_channels,
            driver_input_fallback_channels,
        )
    }

    pub(super) fn prepare_with_selected_device(
        &self,
        output_device: String,
    ) -> Result<PipelinePlan, String> {
        self.pipeline.prepare_with_selected_device(output_device)
    }

    pub(super) fn commit_applied(&mut self, plan: &PipelinePlan) {
        self.pipeline.commit_applied(plan);
    }

    pub(super) fn set_desired_output_device(
        &mut self,
        output_device: Option<String>,
    ) -> Result<(), String> {
        self.pipeline.set_desired_output_device(output_device)
    }

    pub(super) fn commit_idle_reconfigure(&mut self, plan: &PipelinePlan) {
        self.pipeline.commit_idle_reconfigure(plan);
    }
}

/// Spawn a background thread that polls the driver for config changes
pub(super) fn spawn_driver_config_watcher(
    driver_manager: Arc<Mutex<DriverManager>>,
    audio_manager: Arc<Mutex<AudioEngineManager>>,
    running: Arc<Mutex<bool>>,
    system_state: Arc<Mutex<SystemwideState>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        use std::time::Duration;

        let poll_interval = Duration::from_millis(50);

        log::info!("Driver config watcher thread started");

        loop {
            if !*running.lock() {
                break;
            }

            // Poll driver for config changes
            let config_change = driver_manager.lock().poll_config_change();
            if let Some(config) = config_change {
                handle_driver_config_change(&driver_manager, &audio_manager, config, &system_state);
            }

            std::thread::sleep(poll_interval);
        }

        log::info!("Driver config watcher thread stopped");
    })
}
