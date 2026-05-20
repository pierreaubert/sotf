// ============================================================================
// Band Merge Plugin
// ============================================================================

pub mod params;

use serde::{Deserialize, Serialize};
use sotf_host::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use sotf_host::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use sotf_host::simd::{enable_ftz_daz, flush_denormals_inplace};
use std::cell::Cell;

/// Maximum number of bands supported.
const MAX_BANDS: usize = 32;

/// One-pole gain smoother time constant in milliseconds.
/// At 10 ms the gain reaches ~63% of a step change in ~10 ms,
/// which is fast enough for automation while eliminating zipper noise.
const GAIN_SMOOTH_MS: f32 = 10.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandMergePluginParams {
    #[serde(default = "default_num_bands")]
    pub bands: usize,
    /// Per-band gain in dB. Defaults to 0.0 (unity) for each band.
    #[serde(default)]
    pub band_gains_db: Vec<f32>,
    /// Per-band mute flags. Defaults to false (unmuted) for each band.
    #[serde(default)]
    pub band_mutes: Vec<bool>,
}

fn default_num_bands() -> usize {
    2
}

pub struct BandMergePlugin {
    output_channels: usize,
    num_bands: usize,
    param_bands: ParameterId,
    /// Per-band gain in dB (up to MAX_BANDS).
    band_gains_db: [f32; MAX_BANDS],
    /// Per-band linear gain (precomputed from dB).
    band_gains_linear: [f32; MAX_BANDS],
    /// Per-band mute toggle.
    band_mutes: [bool; MAX_BANDS],
    /// Reconstruction error in dB (diagnostic). Measures how much the output
    /// deviates from perfect reconstruction.
    ///
    /// This value is refreshed when requested by the host.
    reconstruction_error_db: f32,
    reconstruction_error_requested: Cell<bool>,
    cached_parameters: Vec<Parameter>,
    // ---- gain smoothing ----
    /// Per-band one-pole gain smoother to prevent zipper noise during automation.
    band_gain_smoothers: [sotf_host::smoothing::Smoother; MAX_BANDS],
    /// Sample rate, needed to reinitialise smoothers on initialize().
    sample_rate: u32,
}

impl BandMergePlugin {
    pub fn new(output_channels: usize, bands: usize) -> Result<Self, String> {
        if bands < 2 {
            return Err("Min 2 bands".into());
        }
        if bands > MAX_BANDS {
            return Err(format!("Max {} bands", MAX_BANDS));
        }
        // Default sample rate before initialize() is called.
        const DEFAULT_SR: u32 = 48000;
        let mut p = Self {
            output_channels,
            num_bands: bands,
            param_bands: ParameterId("bands".to_string()),
            band_gains_db: [0.0; MAX_BANDS],
            band_gains_linear: [1.0; MAX_BANDS],
            band_mutes: [false; MAX_BANDS],
            reconstruction_error_db: 0.0,
            reconstruction_error_requested: Cell::new(false),
            cached_parameters: Vec::new(),
            band_gain_smoothers: std::array::from_fn(|_| {
                sotf_host::smoothing::Smoother::new(1.0, GAIN_SMOOTH_MS, DEFAULT_SR)
            }),
            sample_rate: DEFAULT_SR,
        };
        p.rebuild_cached_parameters();
        Ok(p)
    }

    pub fn from_params(
        output_channels: usize,
        params: &BandMergePluginParams,
    ) -> Result<Self, String> {
        let mut p = Self::new(output_channels, params.bands)?;
        for (i, &g) in params.band_gains_db.iter().enumerate().take(params.bands) {
            p.band_gains_db[i] = g;
            p.band_gains_linear[i] = db_to_linear(g);
            // Snap smoother to the preset value immediately (no ramp on load).
            p.band_gain_smoothers[i].reset(db_to_linear(g));
        }
        for (i, &m) in params.band_mutes.iter().enumerate().take(params.bands) {
            p.band_mutes[i] = m;
        }
        p.rebuild_cached_parameters();
        Ok(p)
    }

    fn rebuild_cached_parameters(&mut self) {
        let mut params = vec![Parameter::new_int(
            "bands",
            "Bands",
            self.num_bands as i32,
            2,
            MAX_BANDS as i32,
        )];
        for i in 0..self.num_bands {
            let gain_id = format!("band_{}_gain_db", i);
            let gain_label = format!("Band {} Gain (dB)", i);
            params.push(Parameter::new_float(
                &gain_id,
                &gain_label,
                self.band_gains_db[i],
                -60.0,
                24.0,
            ));
            let mute_id = format!("band_{}_mute", i);
            let mute_label = format!("Band {} Mute", i);
            params.push(Parameter::new_bool(
                &mute_id,
                &mute_label,
                self.band_mutes[i],
            ));
        }
        params.push(
            Parameter::new_float(
                "reconstruction_error_db",
                "Reconstruction Error",
                self.reconstruction_error_db,
                -60.0,
                60.0,
            )
            .with_description("Deviation from perfect reconstruction in dB (read-only diagnostic)")
            .with_group("Diagnostics")
            .with_importance(ParameterImportance::FineTuning),
        );
        self.cached_parameters = params;
    }
}

#[inline]
fn db_to_linear(db: f32) -> f32 {
    sotf_host::db_to_linear(db)
}

impl Plugin for BandMergePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("BandMerge", "1.1.0", "Sotf")
    }
    fn input_channels(&self) -> usize {
        self.output_channels * self.num_bands
    }
    fn output_channels(&self) -> usize {
        self.output_channels
    }
    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }
    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_bands {
            let v = value.as_int().ok_or("val")? as usize;
            if !(2..=MAX_BANDS).contains(&v) {
                return Err(format!("bands must be 2..={}", MAX_BANDS));
            }
            self.num_bands = v;
            self.rebuild_cached_parameters();
            return Ok(());
        }
        // Check per-band parameters using prefix parsing (no heap allocation)
        if let Some(rest) = id.0.strip_prefix("band_") {
            if let Some(idx_str) = rest.strip_suffix("_gain_db") {
                if let Ok(i) = idx_str.parse::<usize>()
                    && i < self.num_bands
                {
                    let v = value
                        .as_float()
                        .ok_or_else(|| format!("band_{}_gain_db must be a float", i))?;
                    if v.is_finite() {
                        self.band_gains_db[i] = v;
                        let linear = db_to_linear(v);
                        self.band_gains_linear[i] = linear;
                        // Update smoother target so gain transitions are glitch-free.
                        self.band_gain_smoothers[i].set_target(linear);
                        self.rebuild_cached_parameters();
                    }
                    return Ok(());
                }
            } else if let Some(idx_str) = rest.strip_suffix("_mute")
                && let Ok(i) = idx_str.parse::<usize>()
                && i < self.num_bands
            {
                let v = value
                    .as_bool()
                    .ok_or_else(|| format!("band_{}_mute must be a bool", i))?;
                self.band_mutes[i] = v;
                self.rebuild_cached_parameters();
                return Ok(());
            }
        }
        Err(format!("Unknown parameter: {}", id))
    }
    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_bands {
            return Some(ParameterValue::Int(self.num_bands as i32));
        }
        if id.0 == "reconstruction_error_db" {
            self.reconstruction_error_requested.set(true);
            return Some(ParameterValue::Float(self.reconstruction_error_db));
        }
        if let Some(rest) = id.0.strip_prefix("band_") {
            if let Some(idx_str) = rest.strip_suffix("_gain_db") {
                if let Ok(i) = idx_str.parse::<usize>()
                    && i < self.num_bands
                {
                    return Some(ParameterValue::Float(self.band_gains_db[i]));
                }
            } else if let Some(idx_str) = rest.strip_suffix("_mute")
                && let Ok(i) = idx_str.parse::<usize>()
                && i < self.num_bands
            {
                return Some(ParameterValue::Bool(self.band_mutes[i]));
            }
        }
        None
    }
    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        for i in 0..MAX_BANDS {
            self.band_gain_smoothers[i].set_time(GAIN_SMOOTH_MS, sample_rate);
        }
        Ok(())
    }
    fn reset(&mut self) {
        // Snap all smoothers to their current target so playback resumes
        // without a ramp artefact after a transport reset.
        for i in 0..self.num_bands {
            self.band_gain_smoothers[i].reset(self.band_gains_linear[i]);
        }
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Result<usize, String> {
        enable_ftz_daz();
        let num_frames = context.num_frames;
        let out_ch = self.output_channels;
        let in_ch = out_ch * self.num_bands;

        // Accumulate RMS for reconstruction validation:
        // - reference_rms: sum of all bands (unity gain, no mute) -- what perfect reconstruction would be
        // - output_rms: actual summed output with gains and mutes applied
        let measure_reconstruction_error = self.reconstruction_error_requested.replace(false);
        let mut reference_energy = 0.0_f64;
        let mut output_energy = 0.0_f64;

        // Pre-compute effective gains for this frame block:
        // muted bands have effective gain 0.0, which eliminates the per-sample branch
        // and allows the inner loop to be auto-vectorized by the compiler.
        let mut effective_gains = [0.0f32; MAX_BANDS];
        for (band, eg) in effective_gains.iter_mut().enumerate().take(self.num_bands) {
            // Advance the smoother by `num_frames` steps. Using next_n() is
            // equivalent to advancing sample-by-sample for the purpose of the
            // block-wise smoother, and avoids calling it inside the innermost loop.
            let smoothed = self.band_gain_smoothers[band].next_n(num_frames);
            *eg = if self.band_mutes[band] { 0.0 } else { smoothed };
        }

        for frame in 0..num_frames {
            let in_off = frame * in_ch;
            let out_off = frame * out_ch;
            for ch in 0..out_ch {
                let mut sum = 0.0f32;
                let mut ref_sum = 0.0f32;
                for band in 0..self.num_bands {
                    let sample = input[in_off + band * out_ch + ch];
                    if measure_reconstruction_error {
                        ref_sum += sample;
                    }
                    sum += sample * effective_gains[band];
                }
                output[out_off + ch] = sum;
                if measure_reconstruction_error {
                    reference_energy += (ref_sum as f64) * (ref_sum as f64);
                    output_energy += (sum as f64) * (sum as f64);
                }
            }
        }

        // Compute reconstruction error in dB
        if measure_reconstruction_error {
            let total_samples = (num_frames * out_ch) as f64;
            let ref_rms = (reference_energy / total_samples).sqrt();
            let out_rms = (output_energy / total_samples).sqrt();

            if ref_rms > 1e-10 {
                let ratio_db = 20.0 * (out_rms / ref_rms).log10();
                self.reconstruction_error_db = ratio_db as f32;

                if ratio_db.abs() > 3.0 {
                    log::warn!(
                        "[BandMerge] Reconstruction error: {:.1} dB deviation from unity-gain sum. \
                         Check band gains and mute settings.",
                        ratio_db
                    );
                }
            } else {
                // Reference is silence -- no meaningful error to report
                self.reconstruction_error_db = 0.0;
            }
        }

        flush_denormals_inplace(output);
        Ok(num_frames)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_band_merge_basic() {
        let mut p = BandMergePlugin::new(2, 2).unwrap();
        let i = vec![1.0, 2.0, 3.0, 4.0];
        let mut o = vec![0.0, 0.0];
        p.process(&i, &mut o, &ProcessContext::new(48000, 1))
            .unwrap();
        assert_eq!(o, vec![4.0, 6.0]);
    }

    #[test]
    fn test_band_merge_with_gain() {
        let mut p = BandMergePlugin::new(1, 2).unwrap();
        p.initialize(48000).unwrap();
        // Set band 0 gain to +6 dB (~2x), band 1 gain stays at 0 dB (1x)
        p.set_parameter(
            ParameterId::from("band_0_gain_db"),
            ParameterValue::Float(6.0206),
        )
        .unwrap();

        // Process in small blocks to let the smoother converge across many calls.
        // The smoother advances `num_frames` steps per process() call, so the
        // total convergence is: 100 calls × 128 frames = 12 800 steps (>>480).
        let block = 128usize;
        let i_block = vec![1.0f32; block * 2]; // band0=1.0, band1=1.0
        let mut o_block = vec![0.0f32; block];
        let mut last = 0.0f32;
        for _ in 0..100 {
            p.process(&i_block, &mut o_block, &ProcessContext::new(48000, block))
                .unwrap();
            last = o_block[block - 1];
        }
        // After settling: Band 0 * 2.0 + Band 1 * 1.0 = 3.0
        assert!((last - 3.0).abs() < 0.01, "got {last}");
    }

    #[test]
    fn test_band_merge_with_mute() {
        let mut p = BandMergePlugin::new(2, 2).unwrap();
        // Mute band 1
        p.set_parameter(ParameterId::from("band_1_mute"), ParameterValue::Bool(true))
            .unwrap();

        let i = vec![1.0, 2.0, 3.0, 4.0];
        let mut o = vec![0.0, 0.0];
        p.process(&i, &mut o, &ProcessContext::new(48000, 1))
            .unwrap();
        // Only band 0 contributes: [1.0, 2.0]
        assert_eq!(o, vec![1.0, 2.0]);
    }

    #[test]
    fn test_band_merge_mute_and_gain_combined() {
        let mut p = BandMergePlugin::new(1, 3).unwrap();
        p.initialize(48000).unwrap();
        // Mute band 0
        p.set_parameter(ParameterId::from("band_0_mute"), ParameterValue::Bool(true))
            .unwrap();
        // Set band 2 gain to +6 dB (~2x)
        p.set_parameter(
            ParameterId::from("band_2_gain_db"),
            ParameterValue::Float(6.0206),
        )
        .unwrap();

        // Process in small blocks (see test_band_merge_with_gain for rationale).
        // 3 bands, 1 channel: band0=10.0, band1=1.0, band2=1.0 per frame.
        let block = 128usize;
        let frame = [10.0f32, 1.0, 1.0];
        let i_block: Vec<f32> = frame.iter().copied().cycle().take(block * 3).collect();
        let mut o_block = vec![0.0f32; block];
        let mut last = 0.0f32;
        for _ in 0..100 {
            p.process(&i_block, &mut o_block, &ProcessContext::new(48000, block))
                .unwrap();
            last = o_block[block - 1];
        }
        // After settling: band0 muted, band1 * 1.0 + band2 * 2.0 = 3.0
        assert!((last - 3.0).abs() < 0.01, "got {last}");
    }

    #[test]
    fn test_band_merge_get_set_parameters() {
        let mut p = BandMergePlugin::new(2, 3).unwrap();

        // Default gain is 0.0
        assert_eq!(
            p.get_parameter(&ParameterId::from("band_0_gain_db")),
            Some(ParameterValue::Float(0.0))
        );
        // Default mute is false
        assert_eq!(
            p.get_parameter(&ParameterId::from("band_1_mute")),
            Some(ParameterValue::Bool(false))
        );

        // Set and retrieve
        p.set_parameter(
            ParameterId::from("band_2_gain_db"),
            ParameterValue::Float(-3.0),
        )
        .unwrap();
        assert_eq!(
            p.get_parameter(&ParameterId::from("band_2_gain_db")),
            Some(ParameterValue::Float(-3.0))
        );

        p.set_parameter(ParameterId::from("band_0_mute"), ParameterValue::Bool(true))
            .unwrap();
        assert_eq!(
            p.get_parameter(&ParameterId::from("band_0_mute")),
            Some(ParameterValue::Bool(true))
        );
    }

    #[test]
    fn test_band_merge_from_params() {
        let params = BandMergePluginParams {
            bands: 3,
            band_gains_db: vec![6.0206, 0.0, -60.0],
            band_mutes: vec![false, true, false],
        };
        let mut p = BandMergePlugin::from_params(1, &params).unwrap();

        // band0=1.0 * ~2.0, band1=1.0 muted, band2=1.0 * ~0.001
        let i = vec![1.0, 1.0, 1.0];
        let mut o = vec![0.0];
        p.process(&i, &mut o, &ProcessContext::new(48000, 1))
            .unwrap();
        // ~2.0 + 0 + ~0.001 ≈ 2.001
        assert!((o[0] - 2.0).abs() < 0.05, "got {}", o[0]);
    }

    /// With all bands at unity gain (0 dB) and no mutes, the reconstruction
    /// error should be near 0 dB.
    #[test]
    fn test_reconstruction_error_db_unity() {
        let mut p = BandMergePlugin::new(2, 3).unwrap();

        // Process with non-trivial signal
        let nf = 100;
        let in_ch = 2 * 3; // 2 output channels * 3 bands
        let out_ch = 2;
        let mut input = vec![0.0f32; nf * in_ch];
        for frame in 0..nf {
            for band in 0..3 {
                for ch in 0..2 {
                    input[frame * in_ch + band * out_ch + ch] =
                        0.3 * ((frame * 3 + band) as f32 * 0.1).sin();
                }
            }
        }
        let mut output = vec![0.0f32; nf * out_ch];
        let _ = p.get_parameter(&ParameterId::from("reconstruction_error_db"));
        p.process(&input, &mut output, &ProcessContext::new(48000, nf))
            .unwrap();

        // Get reconstruction_error_db via get_parameter
        let err = p
            .get_parameter(&ParameterId::from("reconstruction_error_db"))
            .unwrap();
        if let ParameterValue::Float(err_db) = err {
            assert!(
                err_db.abs() < 0.1,
                "With unity gains and no mutes, reconstruction error should be near 0 dB, got {err_db:.4}"
            );
        } else {
            panic!("reconstruction_error_db should be a Float parameter");
        }
    }

    #[test]
    fn test_reconstruction_error_db_is_computed_on_demand() {
        let mut p = BandMergePlugin::new(1, 2).unwrap();

        // Set a non-unity gain to make the diagnostic value clearly non-zero.
        p.set_parameter(
            ParameterId::from("band_0_gain_db"),
            ParameterValue::Float(6.0206),
        )
        .unwrap();
        p.reset();

        let input = vec![1.0f32, 1.0];
        let mut output = vec![0.0f32];
        p.process(&input, &mut output, &ProcessContext::new(48000, 1))
            .unwrap();

        // No diagnostic read was requested yet, so the value should still be the
        // default 0 dB in-place (no on-demand work performed this frame).
        let err_before = match p
            .get_parameter(&ParameterId::from("reconstruction_error_db"))
            .unwrap()
        {
            ParameterValue::Float(v) => v,
            _ => panic!("reconstruction_error_db should be a Float parameter"),
        };
        assert!(
            err_before.abs() < 0.0001,
            "expected on-demand metric to remain untouched before request-processing cycle, got {err_before}"
        );

        // Next process should perform the diagnostic now that the host requested it.
        p.process(&input, &mut output, &ProcessContext::new(48000, 1))
            .unwrap();

        let err_after = match p
            .get_parameter(&ParameterId::from("reconstruction_error_db"))
            .unwrap()
        {
            ParameterValue::Float(v) => v,
            _ => panic!("reconstruction_error_db should be a Float parameter"),
        };
        assert!(
            err_after.abs() > 1.0,
            "expected reconstructed-error metric to be calculated after request, got {err_after}"
        );
    }

    #[test]
    fn test_band_merge_parameters_list() {
        let p = BandMergePlugin::new(2, 3).unwrap();
        let params = p.parameters();
        // 1 (bands) + 3 * 2 (gain + mute per band) + 1 (reconstruction_error_db) = 8
        assert_eq!(params.len(), 8);
    }

    /// Gain changes must be smoothed: after a step change from 0 dB to +6 dB,
    /// the output on the very first frame must be between the old gain (1.0)
    /// and the new gain (~2.0), not equal to 2.0.  This verifies there is no
    /// step discontinuity (zipper noise) on the first processed frame.
    #[test]
    fn test_gain_change_is_smoothed() {
        let mut p = BandMergePlugin::new(1, 2).unwrap();
        // initialize() sets the smoother coefficient for 48 kHz.
        p.initialize(48000).unwrap();

        // Band gains start at 0 dB (linear 1.0).  Process one frame to lock
        // the smoother at 1.0 (unity).
        let i_unity = vec![1.0f32, 1.0]; // band0=1.0, band1=1.0
        let mut o = vec![0.0f32];
        p.process(&i_unity, &mut o, &ProcessContext::new(48000, 1))
            .unwrap();
        assert!((o[0] - 2.0).abs() < 1e-4, "baseline unity: got {}", o[0]);

        // Now apply a +6 dB step to band 0.  The linear gain jumps from 1.0 → ~2.0.
        p.set_parameter(
            ParameterId::from("band_0_gain_db"),
            ParameterValue::Float(6.0206),
        )
        .unwrap();

        // First frame after the step: the smoother must NOT have reached 2.0 yet.
        // At 48 kHz with a 10 ms time constant the gain after 1 sample is
        // approximately 1.002 — strictly between 1.0 and 2.0.
        let mut o_step = vec![0.0f32];
        p.process(&i_unity, &mut o_step, &ProcessContext::new(48000, 1))
            .unwrap();

        // band0_smoothed (≈1.002) + band1_gain (1.0) = ≈2.002, well below 3.0
        assert!(
            o_step[0] > 2.0 && o_step[0] < 2.1,
            "expected smoothed output between 2.0 and 2.1 on first frame after gain step, got {}",
            o_step[0]
        );
    }

    /// reset() must snap smoothers to their current target immediately so that
    /// playback resumes at the correct gain without an unwanted ramp.
    #[test]
    fn test_reset_snaps_smoother() {
        // Minimum 2 bands required by the plugin.
        // Both bands will have input 1.0; band 0 gets +6 dB (~2.0 linear), band 1 stays at 0 dB.
        let mut p = BandMergePlugin::new(1, 2).unwrap();
        p.initialize(48000).unwrap();

        // Apply a gain that the smoother has not yet reached.
        p.set_parameter(
            ParameterId::from("band_0_gain_db"),
            ParameterValue::Float(6.0206), // ~2.0 linear
        )
        .unwrap();

        // reset() should snap the smoother to 2.0 immediately.
        p.reset();

        // First frame after reset must already be at the target gain.
        // Input: [band0=1.0, band1=1.0]; expected output: 2.0*1.0 + 1.0*1.0 = 3.0.
        let i = vec![1.0f32, 1.0]; // 2 bands, 1 channel, 1 frame
        let mut o = vec![0.0f32];
        p.process(&i, &mut o, &ProcessContext::new(48000, 1))
            .unwrap();
        assert!(
            (o[0] - 3.0).abs() < 0.01,
            "after reset(), first frame should equal settled output (~3.0), got {}",
            o[0]
        );
    }
}
