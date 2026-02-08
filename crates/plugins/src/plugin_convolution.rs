// ============================================================================
// Convolution Plugin - FFT-based convolution for reverb and IR processing
// ============================================================================

use super::param_specs::convolution::*;
use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use super::simd::flush_denormals_inplace;
use super::smoothing::Smoother;
use parking_lot::RwLock;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::conv::FromSample;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;

// ============================================================================
// Configuration
// ============================================================================

fn default_ir_file() -> String {
    String::new()
}

fn default_mix() -> f32 {
    MIX_DEFAULT
}

fn default_gain_db() -> f32 {
    GAIN_DB_DEFAULT
}

/// Configuration parameters for ConvolutionPlugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvolutionPluginParams {
    #[serde(default = "default_ir_file")]
    pub ir_file: String,
    #[serde(default = "default_mix")]
    pub mix: f32,
    #[serde(default = "default_gain_db")]
    pub gain_db: f32,
}

// ============================================================================
// Plugin Implementation
// ============================================================================

struct ConvolutionState {
    ir_fft: Vec<Vec<Complex<f32>>>, // [channel][bin]
    ir_channels: usize,
    fft_size: usize,
    hop_size: usize,
    fft_forward: Arc<dyn rustfft::Fft<f32>>,
    fft_inverse: Arc<dyn rustfft::Fft<f32>>,
}

/// FFT-based convolution plugin for impulse response processing
///
/// Uses overlap-add FFT convolution for efficient processing of long IRs.
/// Supports mono and stereo impulse responses.
///
/// # Threading Model
/// The `state` field uses `Arc<RwLock>` to support async IR loading via `load_ir(sync=false)`.
/// `process()` takes a read lock; `load_ir()` takes a write lock from a rayon background thread.
/// Currently only sync loading is used (at init time), but the async path is preserved for
/// future use (e.g., IR hot-swap during playback).
pub struct ConvolutionPlugin {
    channels: usize,
    sample_rate: u32,

    // Parameters
    #[allow(dead_code)]
    param_ir_file: ParameterId,
    param_mix: ParameterId,
    param_gain_db: ParameterId,

    ir_file: String,

    // Smoothed parameters
    mix: Smoother,
    gain_db: f32, // Stored for get_parameter
    gain_linear: Smoother,

    // Shared state (swapped when new IR is loaded)
    state: Arc<RwLock<Option<ConvolutionState>>>,

    // Overlap-add buffers (per channel)
    // These need to be resized if FFT size changes
    input_buffer: Vec<Vec<f32>>,
    input_buffer_fill: Vec<usize>,
    output_accumulator: Vec<Vec<f32>>,
    output_accumulator_fill: usize,

    // Processing buffers (reused to avoid allocations, resized as needed)
    fft_buffer: Vec<Complex<f32>>,
    conv_result: Vec<Complex<f32>>,

    // Scratch buffers for de-interleaving (per channel)
    scratch_input: Vec<Vec<f32>>,
    scratch_output: Vec<Vec<f32>>,
}

impl ConvolutionPlugin {
    /// Create a new convolution plugin
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        // Initial dummy state
        let fft_size = 2048;

        Self {
            channels,
            sample_rate,

            param_ir_file: ParameterId::from("ir_file"),
            param_mix: ParameterId::from("mix"),
            param_gain_db: ParameterId::from("gain_db"),

            ir_file: String::new(),
            mix: Smoother::new(1.0, 20.0, sample_rate),
            gain_db: 0.0,
            gain_linear: Smoother::new(1.0, 20.0, sample_rate),

            state: Arc::new(RwLock::new(None)),

            // Initialize buffers with default size, will be resized on load
            input_buffer: vec![vec![0.0; fft_size]; channels],
            input_buffer_fill: vec![0; channels],
            output_accumulator: vec![vec![0.0; fft_size * 2]; channels],
            output_accumulator_fill: 0,

            fft_buffer: vec![Complex::new(0.0, 0.0); fft_size],
            conv_result: vec![Complex::new(0.0, 0.0); fft_size],

            scratch_input: vec![Vec::with_capacity(1024); channels],
            scratch_output: vec![Vec::with_capacity(1024); channels],
        }
    }

    /// Create from configuration parameters
    pub fn from_params(
        channels: usize,
        sample_rate: u32,
        params: ConvolutionPluginParams,
    ) -> Result<Self, String> {
        let mut plugin = Self::new(channels, sample_rate);
        plugin.set_mix(params.mix);
        plugin.set_gain_db(params.gain_db);

        if !params.ir_file.is_empty() {
            // Synchronous load for initialization
            plugin.load_ir(&params.ir_file, true)?;
        }

        Ok(plugin)
    }

    /// Load impulse response from a WAV file
    /// sync: true for blocking load (init), false for background load
    pub fn load_ir(&mut self, path: &str, sync: bool) -> Result<(), String> {
        if path.is_empty() {
            let mut lock = self.state.write();
            *lock = None;
            self.ir_file = String::new();
            return Ok(());
        }

        self.ir_file = path.to_string();
        let path_owned = path.to_string();
        let state_arc = self.state.clone();

        let task = move || -> Result<(), String> {
            // Load IR using Symphonia
            let ir_samples = Self::load_wav_file(&path_owned)?;
            let ir_channels = ir_samples.len();
            let ir_length = if !ir_samples.is_empty() {
                ir_samples[0].len()
            } else {
                0
            };

            if ir_length == 0 {
                return Ok(());
            }

            // Determine optimal FFT size
            let hop_size = 1024; // Base hop size
            let required_size = ir_length + hop_size;
            let fft_size = required_size.next_power_of_two().max(2048);
            let effective_hop = fft_size / 2;

            // Rebuild FFT planners
            let mut planner = FftPlanner::<f32>::new();
            let fft_forward = planner.plan_fft_forward(fft_size);
            let fft_inverse = planner.plan_fft_inverse(fft_size);

            // Pre-transform IR
            let mut ir_fft = Vec::with_capacity(ir_channels);
            for channel_samples in ir_samples.iter().take(ir_channels) {
                let mut fft_buffer = vec![Complex::new(0.0, 0.0); fft_size];
                for (i, &sample) in channel_samples.iter().enumerate() {
                    fft_buffer[i] = Complex::new(sample, 0.0);
                }
                fft_forward.process(&mut fft_buffer);
                ir_fft.push(fft_buffer);
            }

            let new_state = ConvolutionState {
                ir_fft,
                ir_channels,
                fft_size,
                hop_size: effective_hop,
                fft_forward,
                fft_inverse,
            };

            {
                let mut lock = state_arc.write();
                *lock = Some(new_state);
            }

            Ok(())
        };

        if sync {
            task()?;
        } else {
            rayon::spawn(move || {
                if let Err(e) = task() {
                    log::error!("[Convolution] Async load failed: {}", e);
                }
            });
        }

        Ok(())
    }

    /// Load a WAV file using Symphonia
    fn load_wav_file(path: &str) -> Result<Vec<Vec<f32>>, String> {
        use std::fs::File;
        use symphonia::core::errors::Error as SymphoniaError;

        let file = File::open(Path::new(path))
            .map_err(|e| format!("Failed to open IR file '{}': {}", path, e))?;

        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let format_opts = FormatOptions::default();

        let probe_result = symphonia_format_riff::WavReader::try_new(mss, &format_opts)
            .map_err(|e| format!("Failed to probe IR file: {}", e))?;

        let mut format = probe_result;
        let track = format
            .default_track()
            .ok_or("No default track in IR file")?;

        let channels = track
            .codec_params
            .channels
            .ok_or("No channel info in IR file")?
            .count();

        let codec_params = track.codec_params.clone();
        let decoder_opts = DecoderOptions::default();

        let mut decoder: Box<dyn symphonia::core::codecs::Decoder> = Box::new(
            symphonia_codec_pcm::PcmDecoder::try_new(&codec_params, &decoder_opts)
                .map_err(|e| format!("Failed to create PCM decoder: {}", e))?,
        );

        let mut samples: Vec<Vec<f32>> = vec![Vec::new(); channels];

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(SymphoniaError::ResetRequired) => {
                    break;
                }
                Err(e) => return Err(format!("Error reading packet: {}", e)),
            };

            let decoded = decoder
                .decode(&packet)
                .map_err(|e| format!("Decode error: {}", e))?;

            let duration = decoded.frames();

            for (ch, sample_vec) in samples.iter_mut().enumerate().take(channels) {
                match &decoded {
                    AudioBufferRef::F32(buf) => {
                        for i in 0..duration {
                            sample_vec.push(buf.chan(ch)[i]);
                        }
                    }
                    AudioBufferRef::S32(buf) => {
                        for i in 0..duration {
                            sample_vec.push(<f32 as FromSample<i32>>::from_sample(buf.chan(ch)[i]));
                        }
                    }
                    AudioBufferRef::S16(buf) => {
                        for i in 0..duration {
                            sample_vec.push(<f32 as FromSample<i16>>::from_sample(buf.chan(ch)[i]));
                        }
                    }
                    AudioBufferRef::U8(buf) => {
                        for i in 0..duration {
                            sample_vec.push(<f32 as FromSample<u8>>::from_sample(buf.chan(ch)[i]));
                        }
                    }
                    AudioBufferRef::S24(buf) => {
                        for i in 0..duration {
                            let sample = buf.chan(ch)[i];
                            let sample_f32 = sample.inner() as f32 / 8388608.0;
                            sample_vec.push(sample_f32);
                        }
                    }
                    AudioBufferRef::F64(buf) => {
                        for i in 0..duration {
                            sample_vec.push(buf.chan(ch)[i] as f32);
                        }
                    }
                    _ => return Err("Unsupported audio format for IR".to_string()),
                }
            }
        }

        if samples.is_empty() || samples[0].is_empty() {
            return Err("IR file contains no audio data".to_string());
        }

        Ok(samples)
    }

    /// Set mix (dry/wet) parameter
    pub fn set_mix(&mut self, mix: f32) {
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }

    /// Set gain in dB
    pub fn set_gain_db(&mut self, gain_db: f32) {
        self.gain_db = gain_db;
        self.gain_linear.set_target(10.0_f32.powf(gain_db / 20.0));
    }

    /// Ensure buffers are sized correctly for current FFT size
    fn resize_buffers(&mut self, fft_size: usize) {
        if self.input_buffer[0].len() != fft_size {
            for ch in 0..self.channels {
                self.input_buffer[ch].resize(fft_size, 0.0);
                // Important: clear fill count if resized to avoid invalid indices
                self.input_buffer_fill[ch] = 0;
                self.output_accumulator[ch].resize(fft_size * 2, 0.0);
                self.output_accumulator[ch].fill(0.0); // Safe clear
            }
            self.output_accumulator_fill = 0;
            self.fft_buffer.resize(fft_size, Complex::new(0.0, 0.0));
            self.conv_result.resize(fft_size, Complex::new(0.0, 0.0));
        }
    }

    /// Process one channel with convolution (Decoupled from self to allow shared state usage)
    #[allow(clippy::too_many_arguments)]
    fn process_channel_internal(
        channel: usize,
        input: &[f32],
        output: &mut [f32],
        state: &ConvolutionState,
        input_buffer: &mut [f32],
        input_buffer_fill: &mut usize,
        output_accumulator: &mut [f32],
        output_accumulator_fill: &mut usize,
        fft_buffer: &mut [Complex<f32>],
        conv_result: &mut [Complex<f32>],
        dry_gain: f32,
        wet_gain: f32,
    ) {
        let num_frames = input.len();
        let mut input_pos = 0;
        let mut output_pos = 0;

        let ir_channel = if state.ir_channels == 1 {
            0
        } else {
            channel.min(state.ir_channels - 1)
        };

        while input_pos < num_frames {
            // 1. Fill input buffer
            let space_in_buffer = state.fft_size - *input_buffer_fill;
            let samples_available = num_frames - input_pos;
            let to_copy = space_in_buffer.min(samples_available);

            input_buffer[*input_buffer_fill..*input_buffer_fill + to_copy]
                .copy_from_slice(&input[input_pos..input_pos + to_copy]);

            *input_buffer_fill += to_copy;
            input_pos += to_copy;

            // 2. Process block when buffer is full
            if *input_buffer_fill >= state.hop_size {
                // Copy to FFT buffer and zero-pad
                fft_buffer.fill(Complex::new(0.0, 0.0));
                for i in 0..state.hop_size {
                    fft_buffer[i] = Complex::new(input_buffer[i], 0.0);
                }

                // Forward FFT
                state.fft_forward.process(fft_buffer);

                // Complex multiply with IR using SIMD optimization
                super::simd::complex_mul_simd(conv_result, fft_buffer, &state.ir_fft[ir_channel]);

                // Inverse FFT
                state.fft_inverse.process(conv_result);

                // Scale by 1/N
                let scale = 1.0 / state.fft_size as f32;

                // Overlap-add
                for i in 0..state.fft_size {
                    output_accumulator[i] += conv_result[i].re * scale;
                }

                // Shift input buffer
                input_buffer.copy_within(state.hop_size..state.fft_size, 0);
                input_buffer[state.hop_size..].fill(0.0);
                *input_buffer_fill -= state.hop_size;

                // Track output accumulator fill
                *output_accumulator_fill = (*output_accumulator_fill).max(state.fft_size);
            }

            // 3. Drain output accumulator
            while output_pos < num_frames && *output_accumulator_fill > 0 {
                let to_drain = (num_frames - output_pos).min(*output_accumulator_fill);

                for i in 0..to_drain {
                    let dry = if output_pos + i < input.len() {
                        input[output_pos + i]
                    } else {
                        0.0
                    };
                    let wet = output_accumulator[i];
                    output[output_pos + i] = dry * dry_gain + wet * wet_gain;
                }

                // Shift accumulator
                output_accumulator.copy_within(to_drain..state.fft_size * 2, 0);
                output_accumulator[state.fft_size * 2 - to_drain..].fill(0.0);
                *output_accumulator_fill -= to_drain;
                output_pos += to_drain;
            }
        }
    }
}

impl Plugin for ConvolutionPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Convolution", "1.1.0", "SotF")
            .with_description("FFT-based convolution (Async loading)")
    }

    fn input_channels(&self) -> usize {
        self.channels
    }

    fn output_channels(&self) -> usize {
        self.channels
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        const MIN_SAMPLE_RATE: u32 = 8_000;
        const MAX_SAMPLE_RATE: u32 = 384_000;

        if !(MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&sample_rate) {
            return Err(format!(
                "Invalid sample rate: {} Hz (valid range: {}-{} Hz)",
                sample_rate, MIN_SAMPLE_RATE, MAX_SAMPLE_RATE
            ));
        }

        self.sample_rate = sample_rate;
        self.mix.set_time(20.0, sample_rate);
        self.gain_linear.set_time(20.0, sample_rate);

        // Pre-allocate scratch buffers to max expected frame count.
        // After this, resize() calls in process() are guaranteed no-ops.
        let max_frames = 2048;
        for ch in 0..self.channels {
            self.scratch_input[ch].resize(max_frames, 0.0);
            self.scratch_output[ch].resize(max_frames, 0.0);
        }

        // Reload IR if it was previously loaded (sample rate changed)
        if !self.ir_file.is_empty() {
            let path = self.ir_file.clone();
            self.load_ir(&path, true)?;
        }
        Ok(())
    }

    fn reset(&mut self) {
        for ch in 0..self.channels {
            self.input_buffer[ch].fill(0.0);
            self.input_buffer_fill[ch] = 0;
            self.output_accumulator[ch].fill(0.0);
        }
        self.output_accumulator_fill = 0;
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("mix", "Mix", MIX_DEFAULT, MIX_MIN, MIX_MAX)
                .with_description("Dry/wet mix (0.0 = dry, 1.0 = wet)")
                .with_group("Output")
                .with_importance(ParameterImportance::Useful),
            Parameter::new_float("gain_db", "Gain", GAIN_DB_DEFAULT, GAIN_DB_MIN, GAIN_DB_MAX)
                .with_description("Output gain in dB")
                .with_group("Output")
                .with_importance(ParameterImportance::Useful),
        ]
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        if id == self.param_mix {
            if let Some(mix) = value.as_float() {
                self.set_mix(mix);
                Ok(())
            } else {
                Err("Mix parameter must be a float".to_string())
            }
        } else if id == self.param_gain_db {
            if let Some(gain_db) = value.as_float() {
                self.set_gain_db(gain_db);
                Ok(())
            } else {
                Err("Gain parameter must be a float".to_string())
            }
        } else {
            Err(format!("Unknown parameter: {}", id))
        }
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        if id == &self.param_mix {
            Some(ParameterValue::Float(self.mix.target()))
        } else if id == &self.param_gain_db {
            Some(ParameterValue::Float(self.gain_db))
        } else {
            None
        }
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _context: &ProcessContext,
    ) -> Result<usize, String> {
        if input.len() != output.len() {
            return Err("Input and output buffers must be same size".to_string());
        }

        if !input.len().is_multiple_of(self.channels) {
            return Err(format!(
                "Buffer size {} is not a multiple of channel count {}",
                input.len(),
                self.channels
            ));
        }

        let num_frames = input.len() / self.channels;

        // Initialize output to zero or dry signal
        let dry_mix = 1.0 - self.mix.current();
        if dry_mix > 0.0 {
            for i in 0..num_frames {
                for ch in 0..self.channels {
                    output[i * self.channels + ch] = input[i * self.channels + ch] * dry_mix;
                }
            }
        } else {
            output.fill(0.0);
        }

        // 1. Check for resize requirements without holding lock for long
        let resize_fft_size = {
            let state_guard = self.state.read();
            if let Some(ref state) = *state_guard {
                if self.input_buffer[0].len() != state.fft_size {
                    Some(state.fft_size)
                } else {
                    None
                }
            } else {
                None
            }
        };

        // 2. Resize if needed (mut access to self)
        if let Some(size) = resize_fft_size {
            self.resize_buffers(size);
        }

        for ch in 0..self.channels {
            self.scratch_input[ch].resize(num_frames, 0.0);
            self.scratch_output[ch].resize(num_frames, 0.0);
        }

        // 3. Process
        let state_guard = self.state.read();
        let has_state = state_guard.is_some();

        if let Some(ref state) = *state_guard {
            for ch in 0..self.channels {
                for frame in 0..num_frames {
                    self.scratch_input[ch][frame] = input[frame * self.channels + ch];
                }

                // Process (pure wet signal)
                Self::process_channel_internal(
                    ch,
                    &self.scratch_input[ch],
                    &mut self.scratch_output[ch],
                    state,
                    &mut self.input_buffer[ch],
                    &mut self.input_buffer_fill[ch],
                    &mut self.output_accumulator[ch],
                    &mut self.output_accumulator_fill,
                    &mut self.fft_buffer,
                    &mut self.conv_result,
                    0.0,
                    1.0,
                );
            }
        }

        // 4. Mix Wet to Output (Dry was already added or output was zeroed)
        if has_state {
            for frame in 0..num_frames {
                let gain = self.gain_linear.next();
                let mix = self.mix.next();
                let wet_gain = mix * gain;

                for ch in 0..self.channels {
                    let idx = frame * self.channels + ch;
                    output[idx] += self.scratch_output[ch][frame] * wet_gain;
                }
            }
        } else {
            // Just update smoothers
            for _ in 0..num_frames {
                self.gain_linear.next();
                self.mix.next();
            }
        }

        flush_denormals_inplace(output);

        Ok(num_frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passthrough_without_ir() {
        let mut plugin = ConvolutionPlugin::new(2, 44100);

        let input = vec![1.0, 2.0, 3.0, 4.0]; // 2 frames, 2 channels
        let mut output = vec![0.0; 4];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Without IR, should passthrough
        assert_eq!(output, input);
    }

    #[test]
    fn test_parameter_mix() {
        let mut plugin = ConvolutionPlugin::new(2, 44100);

        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.5))
            .unwrap();

        assert_eq!(plugin.mix.target(), 0.5);

        let value = plugin.get_parameter(&ParameterId::from("mix"));
        assert_eq!(value.unwrap().as_float(), Some(0.5));
    }

    #[test]
    fn test_parameter_gain() {
        let mut plugin = ConvolutionPlugin::new(2, 44100);

        plugin
            .set_parameter(ParameterId::from("gain_db"), ParameterValue::Float(6.0))
            .unwrap();

        assert_eq!(plugin.gain_db, 6.0);
        // Gain linear target should be ~2.0
        assert!((plugin.gain_linear.target() - 1.995).abs() < 0.01);
    }
}
