use super::consts::{HAAS_DELAY_BUF_SIZE, PARAM_SMOOTH_MS};
use super::default::default_haas_delay_ms;
use super::types::MonoToStereoPluginParams;
use crate::params::PARAMS as MS;
#[cfg(test)]
use rustfft::num_complex::Complex;
use sotf_host::param_bridge;
use sotf_host::param_specs::find_by_key as pk;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{
    Plugin, PluginCompileMetadata, PluginCostClass, PluginInfo, PluginResult, ProcessContext,
};
use sotf_host::smoothing::Smoother;

const ALLPASS_SECTIONS: usize = 3;
pub(super) const IDENTITY_RADIUS: f32 = 0.9999;
const IDENTITY_COEFFICIENT: f32 = -0.9999;

#[derive(Clone, Copy, Default)]
struct Allpass2State {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Allpass2State {
    #[inline]
    fn process(&mut self, input: f32, radius: f32, cosine: f32) -> f32 {
        let a1 = -2.0 * radius * cosine;
        let a2 = radius * radius;
        let output = a2 * input + a1 * self.x1 + self.x2 - a1 * self.y1 - a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
        output
    }

    fn prime(&mut self, value: f32) {
        self.x1 = value;
        self.x2 = value;
        self.y1 = value;
        self.y2 = value;
    }
}

pub struct MonoToStereoPlugin {
    pub(super) sample_rate: u32,
    pub(super) freq_dependent: bool,
    pub(super) stereo_width: Smoother,
    pub(super) decor_low_hz: f32,
    pub(super) decor_high_hz: f32,
    section_states: [Allpass2State; ALLPASS_SECTIONS],
    pub(super) section_cosines: [f32; ALLPASS_SECTIONS],
    pub(super) target_radius: f32,
    first_order_x1: f32,
    pub(super) first_order_y1: f32,
    target_first_order_coefficient: f32,
    pub(super) last_input: f32,
    was_duplicate_fast_path: bool,
    initialized: bool,
    pub(super) haas_delay_ms: f32,
    pub(super) haas_delay_samples: usize,
    pub(super) haas_delay_buf: Vec<f32>,
    pub(super) haas_delay_write_pos: usize,
    pub(super) haas_delay_mask: usize,
    pub(super) cached_parameters: Vec<Parameter>,
    #[cfg(test)]
    pub(super) duplicate_fast_path_frames: usize,
    #[cfg(test)]
    pub(super) smoothed_width_frames: usize,
}

impl Default for MonoToStereoPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl MonoToStereoPlugin {
    pub fn new() -> Self {
        let mut plugin = Self {
            sample_rate: 44_100,
            freq_dependent: pk(MS, "freq_dependent").default_bool(),
            stereo_width: Smoother::new(
                pk(MS, "stereo_width").default_f64() as f32,
                PARAM_SMOOTH_MS,
                44_100,
            ),
            decor_low_hz: pk(MS, "decor_low_hz").default_f64() as f32,
            decor_high_hz: pk(MS, "decor_high_hz").default_f64() as f32,
            section_states: [Allpass2State::default(); ALLPASS_SECTIONS],
            section_cosines: [1.0; ALLPASS_SECTIONS],
            target_radius: 0.82,
            first_order_x1: 0.0,
            first_order_y1: 0.0,
            target_first_order_coefficient: 0.0,
            last_input: 0.0,
            was_duplicate_fast_path: false,
            initialized: false,
            haas_delay_ms: default_haas_delay_ms(),
            haas_delay_samples: 0,
            haas_delay_buf: vec![0.0; HAAS_DELAY_BUF_SIZE],
            haas_delay_write_pos: 0,
            haas_delay_mask: HAAS_DELAY_BUF_SIZE - 1,
            cached_parameters: Vec::new(),
            #[cfg(test)]
            duplicate_fast_path_frames: 0,
            #[cfg(test)]
            smoothed_width_frames: 0,
        };
        plugin.prepare_decorrelator();
        plugin.rebuild_cached_parameters();
        plugin
    }

    pub(super) fn param_value(&self, index: usize) -> Option<f64> {
        match index {
            0 => Some(self.stereo_width.target() as f64),
            1 => Some(self.haas_delay_ms as f64),
            2 => Some(self.decor_low_hz as f64),
            3 => Some(self.decor_high_hz as f64),
            4 => Some(if self.freq_dependent { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    pub(super) fn set_param_value(&mut self, index: usize, value: f64) {
        match index {
            0 => self.stereo_width.set_target(value as f32),
            1 => {
                self.haas_delay_ms = value as f32;
                self.update_haas_delay_samples();
            }
            2 => self.decor_low_hz = value as f32,
            3 => self.decor_high_hz = value as f32,
            4 => self.freq_dependent = value > 0.5,
            _ => {}
        }
    }

    pub(super) fn rebuild_cached_parameters(&mut self) {
        self.cached_parameters = param_bridge::build_parameters(MS, |i| self.param_value(i));
    }

    pub fn try_from_params(
        channels: usize,
        params: MonoToStereoPluginParams,
    ) -> Result<Self, String> {
        if channels != 1 {
            return Err(format!(
                "Mono-to-stereo requires 1 input channel, got {channels}"
            ));
        }
        if !params.stereo_width.is_finite() || !(0.0..=1.0).contains(&params.stereo_width) {
            return Err("stereo_width must be finite and in the range 0..=1".to_string());
        }
        if !params.haas_delay_ms.is_finite() || !(0.0..=5.0).contains(&params.haas_delay_ms) {
            return Err("haas_delay_ms must be finite and in the range 0..=5".to_string());
        }
        if !params.decor_low_hz.is_finite() || !(100.0..=500.0).contains(&params.decor_low_hz) {
            return Err("decor_low_hz must be finite and in the range 100..=500".to_string());
        }
        if !params.decor_high_hz.is_finite() || !(1_000.0..=5_000.0).contains(&params.decor_high_hz)
        {
            return Err("decor_high_hz must be finite and in the range 1000..=5000".to_string());
        }
        if params.decor_low_hz >= params.decor_high_hz {
            return Err("decor_low_hz must be below decor_high_hz".to_string());
        }
        let mut plugin = Self::new();
        plugin.stereo_width.set_target(params.stereo_width);
        plugin.freq_dependent = params.freq_dependent;
        plugin.haas_delay_ms = params.haas_delay_ms;
        plugin.decor_low_hz = params.decor_low_hz;
        plugin.decor_high_hz = params.decor_high_hz;
        plugin.prepare_decorrelator();
        plugin.update_haas_delay_samples();
        plugin.rebuild_cached_parameters();
        Ok(plugin)
    }

    pub fn try_from_params_at_sample_rate(
        channels: usize,
        params: MonoToStereoPluginParams,
        sample_rate: u32,
    ) -> Result<Self, String> {
        if sample_rate == 0 {
            return Err("sample rate must be greater than zero".to_string());
        }
        if params.decor_high_hz >= sample_rate as f32 * 0.5 {
            return Err(format!(
                "decor_high_hz must be below Nyquist ({:.1} Hz) at {sample_rate} Hz",
                sample_rate as f32 * 0.5
            ));
        }
        let mut plugin = Self::try_from_params(channels, params)?;
        plugin.sample_rate = sample_rate;
        plugin.stereo_width.set_time(PARAM_SMOOTH_MS, sample_rate);
        plugin.prepare_decorrelator();
        plugin.update_haas_delay_samples();
        Ok(plugin)
    }

    pub fn from_params(channels: usize, params: MonoToStereoPluginParams) -> Self {
        Self::try_from_params(channels, params)
            .expect("MonoToStereoPlugin::from_params received invalid parameters")
    }

    pub(super) fn update_haas_delay_samples(&mut self) {
        let computed = ((self.haas_delay_ms / 1000.0) * self.sample_rate as f32).round() as usize;
        self.haas_delay_samples = computed.min(HAAS_DELAY_BUF_SIZE - 1);
    }

    fn prepare_decorrelator(&mut self) {
        let low = self.decor_low_hz.max(20.0);
        let high = self.decor_high_hz.max(low + 1.0);
        for (index, cosine) in self.section_cosines.iter_mut().enumerate() {
            let t = index as f32 / (ALLPASS_SECTIONS - 1) as f32;
            let frequency = low * (high / low).powf(t);
            let omega = std::f32::consts::TAU * frequency / self.sample_rate as f32;
            *cosine = omega.cos();
        }
        // A high pole radius localizes phase rotation around the requested
        // crossover band, keeping bass below `decor_low_hz` nearly mono.
        self.target_radius = if self.freq_dependent { 0.998 } else { 0.68 };
        let first_order_frequency = if self.freq_dependent {
            (high * 2.0).min(self.sample_rate as f32 * 0.45)
        } else {
            low
        };
        let tangent =
            (std::f32::consts::PI * first_order_frequency / self.sample_rate as f32).tan();
        self.target_first_order_coefficient =
            ((1.0 - tangent) / (1.0 + tangent)).clamp(-0.98, 0.98);
    }

    #[cfg(test)]
    pub(super) fn allpass_response(coefficient: f32, omega: f32) -> Complex<f32> {
        let delay = Complex::from_polar(1.0, -omega);
        (delay - coefficient) / (Complex::new(1.0, 0.0) - delay * coefficient)
    }

    #[cfg(test)]
    pub(super) fn allpass2_response(radius: f32, cosine: f32, omega: f32) -> Complex<f32> {
        let z1 = Complex::from_polar(1.0, -omega);
        let z2 = z1 * z1;
        let a1 = -2.0 * radius * cosine;
        let a2 = radius * radius;
        (Complex::new(a2, 0.0) + z1 * a1 + z2) / (Complex::new(1.0, 0.0) + z1 * a1 + z2 * a2)
    }

    #[inline]
    fn decorrelate(&mut self, input: f32, width: f32) -> f32 {
        let radius = IDENTITY_RADIUS + width * (self.target_radius - IDENTITY_RADIUS);
        let coefficient = IDENTITY_COEFFICIENT
            + width * (self.target_first_order_coefficient - IDENTITY_COEFFICIENT);
        let mut sample = input;
        for (state, cosine) in self.section_states.iter_mut().zip(self.section_cosines) {
            sample = state.process(sample, radius, cosine);
        }
        let output =
            -coefficient * sample + self.first_order_x1 + coefficient * self.first_order_y1;
        self.first_order_x1 = sample;
        self.first_order_y1 = output;
        output
    }

    fn prime_decorrelator(&mut self, value: f32) {
        for state in &mut self.section_states {
            state.prime(value);
        }
        self.first_order_x1 = value;
        self.first_order_y1 = value;
    }

    #[inline]
    fn write_right(&mut self, output: &mut [f32], frame: usize, sample: f32) {
        if self.haas_delay_samples == 0 {
            output[frame * 2 + 1] = sample;
            return;
        }
        self.haas_delay_buf[self.haas_delay_write_pos] = sample;
        let read_pos = (self.haas_delay_write_pos + HAAS_DELAY_BUF_SIZE - self.haas_delay_samples)
            & self.haas_delay_mask;
        output[frame * 2 + 1] = self.haas_delay_buf[read_pos];
        self.haas_delay_write_pos = (self.haas_delay_write_pos + 1) & self.haas_delay_mask;
    }
}

impl Plugin for MonoToStereoPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("MonoToStereo", env!("CARGO_PKG_VERSION"), "Sotf")
    }

    fn input_channels(&self) -> usize {
        1
    }
    fn output_channels(&self) -> usize {
        2
    }

    fn compile_metadata(&self) -> PluginCompileMetadata {
        let mut metadata = PluginCompileMetadata::linear_transform(
            PluginCostClass::Iir,
            None,
            0,
            true,
            true,
            false,
        );
        metadata.boundary = true;
        metadata
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.cached_parameters.clone()
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if self.initialized
            && matches!(
                id.as_str(),
                "decor_low_hz" | "decor_high_hz" | "freq_dependent"
            )
        {
            return Err(format!(
                "parameter '{}' is structural; rebuild the plugin graph to change it",
                id.as_str()
            ));
        }
        param_bridge::set_parameter(MS, &id, &value, |i, v| self.set_param_value(i, v))?;
        self.rebuild_cached_parameters();
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        param_bridge::get_parameter(MS, id, |i| self.param_value(i))
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        if sample_rate == 0 {
            return Err("sample rate must be greater than zero".to_string());
        }
        if self.decor_high_hz >= sample_rate as f32 * 0.5 {
            return Err(format!(
                "decor_high_hz must be below Nyquist ({:.1} Hz) at {sample_rate} Hz",
                sample_rate as f32 * 0.5
            ));
        }
        self.sample_rate = sample_rate;
        self.stereo_width.set_time(PARAM_SMOOTH_MS, sample_rate);
        self.prepare_decorrelator();
        self.update_haas_delay_samples();
        self.reset();
        self.initialized = true;
        Ok(())
    }

    fn reset(&mut self) {
        self.section_states = [Allpass2State::default(); ALLPASS_SECTIONS];
        self.first_order_x1 = 0.0;
        self.first_order_y1 = 0.0;
        self.last_input = 0.0;
        self.was_duplicate_fast_path = false;
        self.stereo_width.reset(self.stereo_width.target());
        self.haas_delay_buf.fill(0.0);
        self.haas_delay_write_pos = 0;
        #[cfg(test)]
        {
            self.duplicate_fast_path_frames = 0;
            self.smoothed_width_frames = 0;
        }
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        let frames = context.num_frames;
        let output_samples = frames
            .checked_mul(2)
            .ok_or_else(|| format!("output frame count {frames} overflows stereo sample count"))?;
        if input.len() != frames {
            return Err(format!(
                "input buffer must contain exactly {frames} samples, got {}",
                input.len()
            ));
        }
        if output.len() != output_samples {
            return Err(format!(
                "output buffer must contain exactly {output_samples} samples, got {}",
                output.len()
            ));
        }

        let settled = (self.stereo_width.current() - self.stereo_width.target()).abs() < 1.0e-5;
        let settled_width = self.stereo_width.target();
        if settled && settled_width == 0.0 && self.haas_delay_samples == 0 {
            for (frame, sample) in input[..frames].iter().copied().enumerate() {
                output[frame * 2] = sample;
                output[frame * 2 + 1] = sample;
            }
            if let Some(last) = input[..frames].last() {
                self.last_input = *last;
            }
            self.was_duplicate_fast_path = true;
            #[cfg(test)]
            {
                self.duplicate_fast_path_frames += frames;
            }
            return Ok(frames);
        }

        if self.was_duplicate_fast_path {
            self.prime_decorrelator(self.last_input);
            self.was_duplicate_fast_path = false;
        }
        for frame in 0..frames {
            let input_sample = input[frame];
            let width = if settled {
                settled_width
            } else {
                #[cfg(test)]
                {
                    self.smoothed_width_frames += 1;
                }
                self.stereo_width.advance()
            };
            let right = if width == 0.0 {
                input_sample
            } else {
                self.decorrelate(input_sample, width)
            };
            output[frame * 2] = input_sample;
            self.write_right(output, frame, right);
            self.last_input = input_sample;
        }
        Ok(frames)
    }

    fn latency_samples(&self) -> usize {
        0
    }
}
