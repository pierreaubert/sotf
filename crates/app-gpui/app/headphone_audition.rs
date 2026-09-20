use super::state::App;
use super::types::{HeadphoneEqStep, PluginUpdateType, Screen};
use sotf_audio_player::controllers::eq_audition::EqAudition;

impl App {
    fn headphone_audition_sample_rate(&self) -> f64 {
        super::state::audio_device::output_sample_rate_for_track(
            self.playback.sample_rate.unwrap_or(48_000),
            self.audio_device_state
                .current_output_device_name
                .as_deref(),
        )
    }

    pub fn preview_headphone_eq(&mut self, corrected: bool) -> Result<(), String> {
        if self.ui_state.current_screen != Screen::HeadphoneEq
            || self.measurement_state.headphone_eq_state.step != HeadphoneEqStep::Listen
        {
            return Err("Audition is available in the Headphone EQ Listen step".into());
        }
        if !self
            .measurement_state
            .headphone_eq_state
            .result_is_current()
        {
            return Err("Run optimization again before auditioning changed inputs".into());
        }
        let sample_rate_hz = self.headphone_audition_sample_rate();
        let headphone = &mut self.measurement_state.headphone_eq_state;
        if let Some(audition) = &mut headphone.audition {
            if audition.sample_rate_hz != sample_rate_hz {
                return Err("Playback sample rate changed; stop preview before retrying".into());
            }
            audition.set_corrected(&mut self.plugin_state.graph, corrected)?;
        } else {
            let filters: Vec<_> = headphone
                .result
                .as_ref()
                .ok_or("No headphone EQ result")?
                .biquads
                .iter()
                .map(|filter| {
                    (
                        filter.filter_type.clone(),
                        filter.freq,
                        filter.q,
                        filter.db_gain,
                    )
                })
                .collect();
            headphone.audition = Some(EqAudition::start(
                &mut self.plugin_state.graph,
                &filters,
                sample_rate_hz,
                corrected,
            )?);
        }
        self.plugin_state.update_state.pending_plugin_update = Some(PluginUpdateType::Structural);
        Ok(())
    }

    pub fn set_headphone_audition_preamp(&mut self, preamp_db: f64) -> Result<(), String> {
        if self.ui_state.current_screen != Screen::HeadphoneEq
            || self.measurement_state.headphone_eq_state.step != HeadphoneEqStep::Listen
            || !self
                .measurement_state
                .headphone_eq_state
                .result_is_current()
        {
            return Err("A current Headphone EQ result is required to adjust audition".into());
        }
        let audition = self
            .measurement_state
            .headphone_eq_state
            .audition
            .as_mut()
            .ok_or("Start audition before adjusting its preamp")?;
        audition.set_preamp(&mut self.plugin_state.graph, preamp_db)?;
        self.plugin_state.update_state.pending_plugin_update = Some(PluginUpdateType::Structural);
        Ok(())
    }

    pub fn stop_headphone_audition(&mut self) -> Result<(), String> {
        if let Some(audition) = &mut self.measurement_state.headphone_eq_state.audition {
            audition.stop(&mut self.plugin_state.graph)?;
            self.plugin_state.update_state.pending_plugin_update =
                Some(PluginUpdateType::Structural);
        }
        Ok(())
    }

    /// Also covers navigation paths that directly change the screen or step.
    pub fn synchronize_headphone_audition(&mut self) -> Result<(), String> {
        self.synchronize_headphone_audition_with_input_check(true)
    }

    pub(crate) fn synchronize_headphone_audition_with_input_check(
        &mut self,
        check_inputs: bool,
    ) -> Result<(), String> {
        let headphone = &self.measurement_state.headphone_eq_state;
        let should_stop = headphone.audition.as_ref().is_some_and(|audition| {
            !audition.is_stopping()
                && !audition.is_canceling()
                && !audition.stop_failed
                && (self.ui_state.current_screen != Screen::HeadphoneEq
                    || headphone.step != HeadphoneEqStep::Listen
                    || (check_inputs && !headphone.result_is_current())
                    || audition.sample_rate_hz != self.headphone_audition_sample_rate())
        });
        if should_stop {
            self.stop_headphone_audition()?;
        }
        if self
            .measurement_state
            .headphone_eq_state
            .audition
            .as_ref()
            .is_some_and(|audition| audition.is_pending())
        {
            self.plugin_state.update_state.pending_plugin_update =
                Some(PluginUpdateType::Structural);
        }
        Ok(())
    }

    /// Returns true if a failed restore requires pausing audio until retry.
    pub fn finish_headphone_audition_update(&mut self, succeeded: bool) -> bool {
        let headphone = &mut self.measurement_state.headphone_eq_state;
        let Some(audition) = &mut headphone.audition else {
            return false;
        };
        let pause = !succeeded && audition.is_stopping();
        if !audition.finish_update(&mut self.plugin_state.graph, succeeded) {
            headphone.audition = None;
        }
        pause
    }
}
