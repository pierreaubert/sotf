// ============================================================================
// Convolution Plugin - FFT-based convolution for reverb and IR processing
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
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
    1.0
}

fn default_gain_db() -> f32 {
    0.0
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

/// FFT-based convolution plugin for impulse response processing
///
/// Uses overlap-add FFT convolution for efficient processing of long IRs.
/// Supports mono and stereo impulse responses.
///
/// # Example
/// ```ignore
/// use sotf_plugins::ConvolutionPlugin;
///
/// let mut conv = ConvolutionPlugin::new(2, 44100);
/// conv.load_ir("reverb.wav").unwrap();
/// ```
pub struct ConvolutionPlugin {
    channels: usize,
    sample_rate: u32,

    // Parameters
    param_ir_file: ParameterId,
    param_mix: ParameterId,
    param_gain_db: ParameterId,

    ir_file: String,
    mix: f32,
    gain_db: f32,
    gain_linear: f32,

    // IR data
    ir_channels: usize,
    ir_samples: Vec<Vec<f32>>, // [channel][sample]
    ir_length: usize,

    // FFT processing
    fft_size: usize,
    hop_size: usize,
    fft_forward: Arc<dyn rustfft::Fft<f32>>,
    fft_inverse: Arc<dyn rustfft::Fft<f32>>,

    // IR in frequency domain (pre-transformed)
    ir_fft: Vec<Vec<Complex<f32>>>, // [channel][bin]

    // Overlap-add buffers (per channel)
    input_buffer: Vec<Vec<f32>>,       // Accumulate input samples
    input_buffer_fill: Vec<usize>,     // How many samples in input buffer
    output_accumulator: Vec<Vec<f32>>, // Overlap-add accumulator
    output_accumulator_fill: usize,    // Valid samples in output accumulator

    // Processing buffers (reused to avoid allocations)
    fft_buffer: Vec<Complex<f32>>,
    conv_result: Vec<Complex<f32>>,
}

impl ConvolutionPlugin {
    /// Create a new convolution plugin
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        // Use a reasonable default FFT size (will be adjusted when IR is loaded)
        let fft_size = 2048;
        let hop_size = fft_size / 2;

        let mut planner = FftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(fft_size);
        let fft_inverse = planner.plan_fft_inverse(fft_size);

        Self {
            channels,
            sample_rate,

            param_ir_file: ParameterId::from("ir_file"),
            param_mix: ParameterId::from("mix"),
            param_gain_db: ParameterId::from("gain_db"),

            ir_file: String::new(),
            mix: 1.0,
            gain_db: 0.0,
            gain_linear: 1.0,

            ir_channels: 0,
            ir_samples: Vec::new(),
            ir_length: 0,

            fft_size,
            hop_size,
            fft_forward,
            fft_inverse,

            ir_fft: Vec::new(),

            input_buffer: vec![vec![0.0; fft_size]; channels],
            input_buffer_fill: vec![0; channels],
            output_accumulator: vec![vec![0.0; fft_size * 2]; channels],
            output_accumulator_fill: 0,

            fft_buffer: vec![Complex::new(0.0, 0.0); fft_size],
            conv_result: vec![Complex::new(0.0, 0.0); fft_size],
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
            plugin.load_ir(&params.ir_file)?;
        }

        Ok(plugin)
    }

    /// Load impulse response from a WAV file
    pub fn load_ir(&mut self, path: &str) -> Result<(), String> {
        if path.is_empty() {
            // Clear IR
            self.ir_file = String::new();
            self.ir_samples.clear();
            self.ir_fft.clear();
            self.ir_length = 0;
            self.ir_channels = 0;
            return Ok(());
        }

        // Load IR using Symphonia
        let ir_data = Self::load_wav_file(path)?;

        self.ir_file = path.to_string();
        self.ir_channels = ir_data.len();
        self.ir_length = if !ir_data.is_empty() {
            ir_data[0].len()
        } else {
            0
        };
        self.ir_samples = ir_data;

        // Determine optimal FFT size (next power of 2 >= IR length + hop size)
        let required_size = self.ir_length + self.hop_size;
        self.fft_size = required_size.next_power_of_two().max(2048);
        self.hop_size = self.fft_size / 2;

        // Rebuild FFT planners
        let mut planner = FftPlanner::<f32>::new();
        self.fft_forward = planner.plan_fft_forward(self.fft_size);
        self.fft_inverse = planner.plan_fft_inverse(self.fft_size);

        // Pre-transform IR to frequency domain
        self.ir_fft = Vec::with_capacity(self.ir_channels);
        for ch in 0..self.ir_channels {
            let mut fft_buffer = vec![Complex::new(0.0, 0.0); self.fft_size];

            // Copy IR samples and zero-pad
            for (i, &sample) in self.ir_samples[ch].iter().enumerate() {
                fft_buffer[i] = Complex::new(sample, 0.0);
            }

            // Transform to frequency domain
            self.fft_forward.process(&mut fft_buffer);
            self.ir_fft.push(fft_buffer);
        }

        // Resize buffers
        self.input_buffer = vec![vec![0.0; self.fft_size]; self.channels];
        self.input_buffer_fill = vec![0; self.channels];
        self.output_accumulator = vec![vec![0.0; self.fft_size * 2]; self.channels];
        self.output_accumulator_fill = 0;
        self.fft_buffer = vec![Complex::new(0.0, 0.0); self.fft_size];
        self.conv_result = vec![Complex::new(0.0, 0.0); self.fft_size];

        log::info!(
            "[Convolution] Loaded IR: {} channels, {} samples, FFT size: {}",
            self.ir_channels,
            self.ir_length,
            self.fft_size
        );

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

        // Use symphonia_format_riff for WAV file support
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

        // Create decoder - use PCM decoder for WAV files
        let codec_params = track.codec_params.clone();
        let decoder_opts = DecoderOptions::default();

        let mut decoder: Box<dyn symphonia::core::codecs::Decoder> = Box::new(
            symphonia_codec_pcm::PcmDecoder::try_new(&codec_params, &decoder_opts)
                .map_err(|e| format!("Failed to create PCM decoder: {}", e))?,
        );

        let mut samples: Vec<Vec<f32>> = vec![Vec::new(); channels];

        // Decode all packets
        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(SymphoniaError::ResetRequired) => {
                    // End of stream
                    break;
                }
                Err(e) => return Err(format!("Error reading packet: {}", e)),
            };

            let decoded = decoder
                .decode(&packet)
                .map_err(|e| format!("Decode error: {}", e))?;

            // Convert to f32 samples
            let duration = decoded.frames();

            for ch in 0..channels {
                match &decoded {
                    AudioBufferRef::F32(buf) => {
                        for i in 0..duration {
                            samples[ch].push(buf.chan(ch)[i]);
                        }
                    }
                    AudioBufferRef::S32(buf) => {
                        for i in 0..duration {
                            samples[ch].push(<f32 as FromSample<i32>>::from_sample(buf.chan(ch)[i]));
                        }
                    }
                    AudioBufferRef::S16(buf) => {
                        for i in 0..duration {
                            samples[ch].push(<f32 as FromSample<i16>>::from_sample(buf.chan(ch)[i]));
                        }
                    }
                    AudioBufferRef::U8(buf) => {
                        for i in 0..duration {
                            samples[ch].push(<f32 as FromSample<u8>>::from_sample(buf.chan(ch)[i]));
                        }
                    }
                    AudioBufferRef::S24(buf) => {
                        for i in 0..duration {
                            // S24 samples need to be converted to i32 first
                            let sample = buf.chan(ch)[i];
                            // Convert i24 to f32 by scaling
                            let sample_f32 = sample.inner() as f32 / 8388608.0; // 2^23
                            samples[ch].push(sample_f32);
                        }
                    }
                    AudioBufferRef::F64(buf) => {
                        for i in 0..duration {
                            samples[ch].push(buf.chan(ch)[i] as f32);
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
        self.mix = mix.clamp(0.0, 1.0);
    }

    /// Set gain in dB
    pub fn set_gain_db(&mut self, gain_db: f32) {
        self.gain_db = gain_db;
        self.gain_linear = 10.0_f32.powf(gain_db / 20.0);
    }

    /// Process one channel with convolution
    fn process_channel(
        &mut self,
        channel: usize,
        input: &[f32],
        output: &mut [f32],
    ) -> Result<(), String> {
        if self.ir_length == 0 {
            // No IR loaded, passthrough
            output.copy_from_slice(input);
            return Ok(());
        }

        let num_frames = input.len();
        let mut input_pos = 0;
        let mut output_pos = 0;

        // Determine which IR channel to use
        let ir_channel = if self.ir_channels == 1 {
            0 // Mono IR: use for all channels
        } else {
            channel.min(self.ir_channels - 1) // Stereo IR: match channels
        };

        while input_pos < num_frames {
            // 1. Fill input buffer
            let space_in_buffer = self.fft_size - self.input_buffer_fill[channel];
            let samples_available = num_frames - input_pos;
            let to_copy = space_in_buffer.min(samples_available);

            self.input_buffer[channel]
                [self.input_buffer_fill[channel]..self.input_buffer_fill[channel] + to_copy]
                .copy_from_slice(&input[input_pos..input_pos + to_copy]);

            self.input_buffer_fill[channel] += to_copy;
            input_pos += to_copy;

            // 2. Process block when buffer is full
            if self.input_buffer_fill[channel] >= self.hop_size {
                // Copy to FFT buffer and zero-pad
                self.fft_buffer.fill(Complex::new(0.0, 0.0));
                for i in 0..self.hop_size {
                    self.fft_buffer[i] = Complex::new(self.input_buffer[channel][i], 0.0);
                }

                // Forward FFT
                self.fft_forward.process(&mut self.fft_buffer);

                // Complex multiply with IR in frequency domain
                for i in 0..self.fft_size {
                    self.conv_result[i] = self.fft_buffer[i] * self.ir_fft[ir_channel][i];
                }

                // Inverse FFT
                self.fft_inverse.process(&mut self.conv_result);

                // Scale by 1/N (IFFT normalization)
                let scale = 1.0 / self.fft_size as f32;

                // Overlap-add into output accumulator
                for i in 0..self.fft_size {
                    self.output_accumulator[channel][i] += self.conv_result[i].re * scale;
                }

                // Shift input buffer
                self.input_buffer[channel].copy_within(self.hop_size..self.fft_size, 0);
                self.input_buffer[channel][self.hop_size..].fill(0.0);
                self.input_buffer_fill[channel] -= self.hop_size;

                // Track output accumulator fill
                self.output_accumulator_fill = self.output_accumulator_fill.max(self.fft_size);
            }

            // 3. Drain output accumulator
            while output_pos < num_frames && self.output_accumulator_fill > 0 {
                let to_drain = (num_frames - output_pos).min(self.output_accumulator_fill);

                // Mix dry/wet
                let wet_gain = self.mix * self.gain_linear;
                let dry_gain = 1.0 - self.mix;

                for i in 0..to_drain {
                    let dry = if output_pos + i < input.len() {
                        input[output_pos + i]
                    } else {
                        0.0
                    };
                    let wet = self.output_accumulator[channel][i];
                    output[output_pos + i] = dry * dry_gain + wet * wet_gain;
                }

                // Shift accumulator
                self.output_accumulator[channel].copy_within(to_drain..self.fft_size * 2, 0);
                self.output_accumulator[channel][self.fft_size * 2 - to_drain..].fill(0.0);
                self.output_accumulator_fill -= to_drain;
                output_pos += to_drain;
            }
        }

        Ok(())
    }
}

impl Plugin for ConvolutionPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "Convolution".to_string(),
            version: "1.0.0".to_string(),
            author: "SOTF".to_string(),
            description: "FFT-based convolution for impulse response processing".to_string(),
        }
    }

    fn input_channels(&self) -> usize {
        self.channels
    }

    fn output_channels(&self) -> usize {
        self.channels
    }

    fn initialize(&mut self, sample_rate: u32) -> PluginResult<()> {
        self.sample_rate = sample_rate;
        // Reload IR if it was previously loaded (sample rate changed)
        if !self.ir_file.is_empty() {
            let path = self.ir_file.clone();
            self.load_ir(&path)?;
        }
        Ok(())
    }

    fn reset(&mut self) {
        // Clear all buffers
        for ch in 0..self.channels {
            self.input_buffer[ch].fill(0.0);
            self.input_buffer_fill[ch] = 0;
            self.output_accumulator[ch].fill(0.0);
        }
        self.output_accumulator_fill = 0;
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("mix", "Mix", 1.0, 0.0, 1.0)
                .with_description("Dry/wet mix (0.0 = dry, 1.0 = wet)"),
            Parameter::new_float("gain_db", "Gain", 0.0, -20.0, 20.0)
                .with_description("Output gain in dB"),
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
            Some(ParameterValue::Float(self.mix))
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
    ) -> PluginResult<()> {
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

        // Process each channel
        for ch in 0..self.channels {
            // Extract channel samples
            let mut channel_input = vec![0.0; num_frames];
            let mut channel_output = vec![0.0; num_frames];

            for frame in 0..num_frames {
                channel_input[frame] = input[frame * self.channels + ch];
            }

            // Process
            self.process_channel(ch, &channel_input, &mut channel_output)?;

            // Interleave back
            for frame in 0..num_frames {
                output[frame * self.channels + ch] = channel_output[frame];
            }
        }

        Ok(())
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

        assert_eq!(plugin.mix, 0.5);

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
        assert!((plugin.gain_linear - 1.995).abs() < 0.01);
    }
}
