use super::pipeline_spec::PipelineSpec;
use super::pipeline_supervisor::PipelineSupervisor;
use super::types::PipelinePlan;
use sotf_audio::PluginConfig;
use sotf_audio::engine::PluginGraphConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PipelineRecovery {
    pub(super) error: String,
    pub(super) actions: Vec<String>,
}

#[derive(Debug, Default)]
pub(super) struct SystemwideState {
    pub(super) pipeline: PipelineSupervisor,
    /// Durable control-plane state for a transition that left the engine
    /// stopped. This is separate from `AppliedPipeline`: the last applied
    /// plan remains authoritative even when a later transition cannot be
    /// applied or restored.
    pub(super) pipeline_recovery: Option<PipelineRecovery>,
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
    pub(super) fn generation(&self) -> u64 {
        self.pipeline.generation()
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

    pub(super) fn pipeline_recovery(&self) -> Option<PipelineRecovery> {
        self.pipeline_recovery.clone()
    }

    pub(super) fn mark_pipeline_recovery(&mut self, error: impl Into<String>) {
        self.pipeline_recovery = Some(PipelineRecovery {
            error: error.into(),
            actions: vec!["restart_daemon".to_string()],
        });
    }

    pub(super) fn clear_pipeline_recovery(&mut self) {
        self.pipeline_recovery = None;
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

    pub(super) fn prepare_from_spec(
        &self,
        spec: PipelineSpec,
        driver_input_fallback_channels: usize,
    ) -> Result<PipelinePlan, String> {
        self.pipeline
            .prepare_from_spec(spec, driver_input_fallback_channels)
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
