// ============================================================================
// Matrix Plugin - Channel mixer with configurable gain matrix
// ============================================================================

use super::parameters::{Parameter, ParameterId, ParameterImportance, ParameterValue};
use super::plugin::{Plugin, PluginInfo, PluginResult, ProcessContext};

/// Matrix mixer plugin that routes N input channels to P output channels
///
/// Each output channel is computed as a weighted sum of all input channels,
/// where the weights are specified by an NxP gain matrix.
///
/// Supports sparse channel mapping where input/output channels can be mapped
/// to non-contiguous physical channel indices. For example:
/// - Input channels [1, 2] means read from physical channels 1 and 2
/// - Output channels [15, 16] means write to physical channels 15 and 16
/// - Total buffer size is determined by max(channel indices) + 1
///
/// # Matrix Layout
/// The matrix is stored in row-major order where:
/// - matrix[out_ch * num_inputs + in_ch] = gain from logical input in_ch to logical output out_ch
///
/// # Examples
///
/// Identity matrix (2x2): stereo pass-through
/// ```text
/// [1.0, 0.0,   // Out0 = 1.0*In0 + 0.0*In1
///  0.0, 1.0]   // Out1 = 0.0*In0 + 1.0*In1
/// ```
///
/// Swap channels (2x2):
/// ```text
/// [0.0, 1.0,   // Out0 = 0.0*In0 + 1.0*In1
///  1.0, 0.0]   // Out1 = 1.0*In0 + 0.0*In1
/// ```
///
/// Sparse mapping (channels 1,2 -> 15,16):
/// ```text
/// input_channel_map = [1, 2]
/// output_channel_map = [15, 16]
/// matrix = [1.0, 0.0,   // PhysOut15 = PhysIn1
///           0.0, 1.0]   // PhysOut16 = PhysIn2
/// ```
pub struct MatrixPlugin {
    /// Mapping from logical input index to physical channel index
    /// Empty vec means dense mapping [0, 1, 2, ..., n-1]
    input_channel_map: Vec<usize>,
    /// Mapping from logical output index to physical channel index
    /// Empty vec means dense mapping [0, 1, 2, ..., p-1]
    output_channel_map: Vec<usize>,
    /// Gain matrix in row-major order (num_outputs x num_inputs)
    /// matrix[out_ch * num_inputs + in_ch] = gain from logical input in_ch to logical output out_ch
    matrix: Vec<f32>,
    /// Total number of physical input channels in the audio buffer
    /// (max(input_channel_map) + 1, or input_channel_map.len() if empty)
    physical_input_channels: usize,
    /// Total number of physical output channels in the audio buffer
    /// (max(output_channel_map) + 1, or output_channel_map.len() if empty)
    physical_output_channels: usize,
}

impl MatrixPlugin {
    /// Create a new matrix plugin with identity matrix (dense mapping)
    ///
    /// # Arguments
    /// * `input_channels` - Number of input channels (dense mapping: 0, 1, 2, ...)
    /// * `output_channels` - Number of output channels (dense mapping: 0, 1, 2, ...)
    ///
    /// The matrix is initialized as identity (1.0 on diagonal, 0.0 elsewhere).
    /// For non-square matrices, only matching indices get 1.0.
    pub fn new(input_channels: usize, output_channels: usize) -> Self {
        let matrix = Self::create_identity_matrix(input_channels, output_channels);
        Self {
            input_channel_map: Vec::new(),  // Empty = dense mapping
            output_channel_map: Vec::new(), // Empty = dense mapping
            matrix,
            physical_input_channels: input_channels,
            physical_output_channels: output_channels,
        }
    }

    /// Create a new matrix plugin with custom matrix (dense mapping)
    ///
    /// # Arguments
    /// * `input_channels` - Number of input channels
    /// * `output_channels` - Number of output channels
    /// * `matrix` - Gain matrix in row-major order (must be output_channels * input_channels elements)
    ///
    /// # Returns
    /// `Ok(plugin)` if matrix size is correct, `Err(message)` otherwise
    pub fn with_matrix(
        input_channels: usize,
        output_channels: usize,
        matrix: Vec<f32>,
    ) -> Result<Self, String> {
        let expected_size = output_channels * input_channels;
        if matrix.len() != expected_size {
            return Err(format!(
                "Matrix size mismatch: expected {} elements ({}x{}), got {}",
                expected_size,
                output_channels,
                input_channels,
                matrix.len()
            ));
        }

        // Validate gains are in valid range [0.0, 1.0]
        for (idx, &gain) in matrix.iter().enumerate() {
            if !(0.0..=1.0).contains(&gain) {
                return Err(format!(
                    "Matrix element {} has invalid gain {:.3} (must be 0.0-1.0)",
                    idx, gain
                ));
            }
        }

        Ok(Self {
            input_channel_map: Vec::new(),  // Empty = dense mapping
            output_channel_map: Vec::new(), // Empty = dense mapping
            matrix,
            physical_input_channels: input_channels,
            physical_output_channels: output_channels,
        })
    }

    /// Create a new matrix plugin with sparse channel mapping
    ///
    /// # Arguments
    /// * `input_channel_map` - Physical channel indices for inputs (e.g., [1, 2])
    /// * `output_channel_map` - Physical channel indices for outputs (e.g., [15, 16])
    /// * `matrix` - Gain matrix in row-major order (output_map.len() * input_map.len() elements)
    ///
    /// # Returns
    /// `Ok(plugin)` if parameters are valid, `Err(message)` otherwise
    ///
    /// # Example
    /// ```
    /// use sotf_plugins::MatrixPlugin;
    ///
    /// // Map physical channels 1,2 to physical channels 15,16 with identity matrix
    /// let plugin = MatrixPlugin::with_sparse_mapping(
    ///     vec![1, 2],    // Read from physical channels 1, 2
    ///     vec![15, 16],  // Write to physical channels 15, 16
    ///     vec![1.0, 0.0, // PhysOut15 = PhysIn1
    ///          0.0, 1.0] // PhysOut16 = PhysIn2
    /// ).unwrap();
    /// ```
    pub fn with_sparse_mapping(
        input_channel_map: Vec<usize>,
        output_channel_map: Vec<usize>,
        matrix: Vec<f32>,
    ) -> Result<Self, String> {
        if input_channel_map.is_empty() {
            return Err("Input channel map cannot be empty".to_string());
        }
        if output_channel_map.is_empty() {
            return Err("Output channel map cannot be empty".to_string());
        }

        let num_inputs = input_channel_map.len();
        let num_outputs = output_channel_map.len();
        let expected_size = num_outputs * num_inputs;

        if matrix.len() != expected_size {
            return Err(format!(
                "Matrix size mismatch: expected {} elements ({}x{}), got {}",
                expected_size,
                num_outputs,
                num_inputs,
                matrix.len()
            ));
        }

        // Validate gains are in valid range [0.0, 1.0]
        for (idx, &gain) in matrix.iter().enumerate() {
            if !(0.0..=1.0).contains(&gain) {
                return Err(format!(
                    "Matrix element {} has invalid gain {:.3} (must be 0.0-1.0)",
                    idx, gain
                ));
            }
        }

        // Calculate physical channel counts
        let physical_input_channels = input_channel_map.iter().max().map(|&v| v + 1).unwrap();
        let physical_output_channels = output_channel_map.iter().max().map(|&v| v + 1).unwrap();

        Ok(Self {
            input_channel_map,
            output_channel_map,
            matrix,
            physical_input_channels,
            physical_output_channels,
        })
    }

    /// Create an identity matrix
    fn create_identity_matrix(input_channels: usize, output_channels: usize) -> Vec<f32> {
        let mut matrix = vec![0.0; output_channels * input_channels];
        let min_channels = input_channels.min(output_channels);
        for i in 0..min_channels {
            matrix[i * input_channels + i] = 1.0;
        }
        matrix
    }

    /// Get the number of logical input channels (not physical channel count)
    fn num_inputs(&self) -> usize {
        if self.input_channel_map.is_empty() {
            self.physical_input_channels
        } else {
            self.input_channel_map.len()
        }
    }

    /// Get the number of logical output channels (not physical channel count)
    fn num_outputs(&self) -> usize {
        if self.output_channel_map.is_empty() {
            self.physical_output_channels
        } else {
            self.output_channel_map.len()
        }
    }

    /// Get the gain from logical input channel to logical output channel
    pub fn get_gain(&self, input_ch: usize, output_ch: usize) -> Option<f32> {
        let num_inputs = self.num_inputs();
        let num_outputs = self.num_outputs();

        if input_ch >= num_inputs || output_ch >= num_outputs {
            return None;
        }
        Some(self.matrix[output_ch * num_inputs + input_ch])
    }

    /// Set the gain from logical input channel to logical output channel
    ///
    /// # Returns
    /// `Ok(())` if successful, `Err(message)` if channels are out of range or gain is invalid
    pub fn set_gain(&mut self, input_ch: usize, output_ch: usize, gain: f32) -> Result<(), String> {
        let num_inputs = self.num_inputs();
        let num_outputs = self.num_outputs();

        if input_ch >= num_inputs {
            return Err(format!(
                "Input channel {} out of range (max {})",
                input_ch,
                num_inputs - 1
            ));
        }
        if output_ch >= num_outputs {
            return Err(format!(
                "Output channel {} out of range (max {})",
                output_ch,
                num_outputs - 1
            ));
        }
        if !(0.0..=1.0).contains(&gain) {
            return Err(format!("Gain {:.3} out of range (must be 0.0-1.0)", gain));
        }

        self.matrix[output_ch * num_inputs + input_ch] = gain;
        Ok(())
    }

    /// Set the entire matrix
    ///
    /// # Arguments
    /// * `matrix` - New gain matrix in row-major order
    ///
    /// # Returns
    /// `Ok(())` if successful, `Err(message)` if matrix size is wrong or gains are invalid
    pub fn set_matrix(&mut self, matrix: Vec<f32>) -> Result<(), String> {
        let num_inputs = self.num_inputs();
        let num_outputs = self.num_outputs();
        let expected_size = num_outputs * num_inputs;

        if matrix.len() != expected_size {
            return Err(format!(
                "Matrix size mismatch: expected {} elements, got {}",
                expected_size,
                matrix.len()
            ));
        }

        // Validate gains
        for (idx, &gain) in matrix.iter().enumerate() {
            if !(0.0..=1.0).contains(&gain) {
                return Err(format!(
                    "Matrix element {} has invalid gain {:.3} (must be 0.0-1.0)",
                    idx, gain
                ));
            }
        }

        self.matrix = matrix;
        Ok(())
    }

    /// Get a reference to the matrix
    pub fn matrix(&self) -> &[f32] {
        &self.matrix
    }

    /// Reset to identity matrix
    pub fn reset_to_identity(&mut self) {
        let num_inputs = self.num_inputs();
        let num_outputs = self.num_outputs();
        self.matrix = Self::create_identity_matrix(num_inputs, num_outputs);
    }
}

impl Plugin for MatrixPlugin {
    fn info(&self) -> PluginInfo {
        let num_inputs = self.num_inputs();
        let num_outputs = self.num_outputs();

        let description = if self.input_channel_map.is_empty() && self.output_channel_map.is_empty()
        {
            format!(
                "Channel matrix mixer ({}x{} channels)",
                num_inputs, num_outputs
            )
        } else {
            let in_map_str = if self.input_channel_map.is_empty() {
                format!("0..{}", num_inputs)
            } else {
                format!("{:?}", self.input_channel_map)
            };
            let out_map_str = if self.output_channel_map.is_empty() {
                format!("0..{}", num_outputs)
            } else {
                format!("{:?}", self.output_channel_map)
            };
            format!(
                "Channel matrix mixer: {} → {} ({}x{})",
                in_map_str, out_map_str, num_inputs, num_outputs
            )
        };

        PluginInfo::new("Matrix", "1.0.0", "SotF").with_description(description)
    }

    fn input_channels(&self) -> usize {
        self.physical_input_channels
    }

    fn output_channels(&self) -> usize {
        self.physical_output_channels
    }

    fn parameters(&self) -> Vec<Parameter> {
        // Create parameters for each matrix element
        let num_inputs = self.num_inputs();
        let num_outputs = self.num_outputs();
        let mut params = Vec::new();

        for out_ch in 0..num_outputs {
            for in_ch in 0..num_inputs {
                let param_id = format!("gain_{}_{}", in_ch, out_ch);
                let param_name = format!("In{} → Out{}", in_ch, out_ch);
                params.push(
                    Parameter::new_float(&param_id, &param_name, 0.0, 0.0, 1.0)
                        .with_description(&format!(
                            "Gain from input channel {} to output channel {}",
                            in_ch, out_ch
                        ))
                        .with_group("Matrix")
                        .with_importance(ParameterImportance::Useful),
                );
            }
        }
        params
    }

    fn set_parameter(&mut self, id: ParameterId, value: ParameterValue) -> PluginResult<()> {
        // Parse parameter ID: "gain_IN_OUT"
        let id_str = id.0.as_str();
        if !id_str.starts_with("gain_") {
            return Err(format!("Unknown parameter: {}", id));
        }

        let parts: Vec<&str> = id_str.split('_').collect();
        if parts.len() != 3 {
            return Err(format!("Invalid parameter format: {}", id));
        }

        let in_ch = parts[1]
            .parse::<usize>()
            .map_err(|_| format!("Invalid input channel: {}", parts[1]))?;
        let out_ch = parts[2]
            .parse::<usize>()
            .map_err(|_| format!("Invalid output channel: {}", parts[2]))?;

        let gain = value
            .as_float()
            .ok_or_else(|| "Parameter value must be a float".to_string())?;

        self.set_gain(in_ch, out_ch, gain)
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        // Parse parameter ID: "gain_IN_OUT"
        let id_str = id.0.as_str();
        if !id_str.starts_with("gain_") {
            return None;
        }

        let parts: Vec<&str> = id_str.split('_').collect();
        if parts.len() != 3 {
            return None;
        }

        let in_ch = parts[1].parse::<usize>().ok()?;
        let out_ch = parts[2].parse::<usize>().ok()?;

        self.get_gain(in_ch, out_ch).map(ParameterValue::Float)
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<()> {
        let num_frames = context.num_frames;

        // Validate buffer sizes
        if input.len() != num_frames * self.physical_input_channels {
            return Err(format!(
                "Input buffer size {} doesn't match expected {} (frames={}, physical_channels={})",
                input.len(),
                num_frames * self.physical_input_channels,
                num_frames,
                self.physical_input_channels
            ));
        }
        if output.len() != num_frames * self.physical_output_channels {
            return Err(format!(
                "Output buffer size {} doesn't match expected {} (frames={}, physical_channels={})",
                output.len(),
                num_frames * self.physical_output_channels,
                num_frames,
                self.physical_output_channels
            ));
        }

        let num_inputs = self.num_inputs();
        let num_outputs = self.num_outputs();

        // Zero output buffer first (needed for sparse mapping)
        output.fill(0.0);

        // Process frame by frame
        for frame in 0..num_frames {
            let in_frame_offset = frame * self.physical_input_channels;
            let out_frame_offset = frame * self.physical_output_channels;

            // Compute each logical output channel
            for logical_out_ch in 0..num_outputs {
                let mut sum = 0.0;

                // Sum contributions from all logical input channels
                for logical_in_ch in 0..num_inputs {
                    let gain = self.matrix[logical_out_ch * num_inputs + logical_in_ch];

                    // Map logical channel to physical channel
                    let physical_in_ch = if self.input_channel_map.is_empty() {
                        logical_in_ch
                    } else {
                        self.input_channel_map[logical_in_ch]
                    };

                    let input_sample = input[in_frame_offset + physical_in_ch];
                    sum += gain * input_sample;
                }

                // Map logical output to physical output
                let physical_out_ch = if self.output_channel_map.is_empty() {
                    logical_out_ch
                } else {
                    self.output_channel_map[logical_out_ch]
                };

                output[out_frame_offset + physical_out_ch] = sum;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_matrix_2x2() {
        let plugin = MatrixPlugin::new(2, 2);
        assert_eq!(plugin.get_gain(0, 0), Some(1.0));
        assert_eq!(plugin.get_gain(1, 1), Some(1.0));
        assert_eq!(plugin.get_gain(0, 1), Some(0.0));
        assert_eq!(plugin.get_gain(1, 0), Some(0.0));
    }

    #[test]
    fn test_identity_matrix_nonsquare() {
        // 2 inputs, 4 outputs
        let plugin = MatrixPlugin::new(2, 4);
        assert_eq!(plugin.get_gain(0, 0), Some(1.0));
        assert_eq!(plugin.get_gain(1, 1), Some(1.0));
        assert_eq!(plugin.get_gain(0, 2), Some(0.0));
        assert_eq!(plugin.get_gain(1, 3), Some(0.0));
    }

    #[test]
    fn test_swap_channels() {
        let mut plugin = MatrixPlugin::new(2, 2);
        plugin.set_gain(0, 0, 0.0).unwrap();
        plugin.set_gain(1, 1, 0.0).unwrap();
        plugin.set_gain(1, 0, 1.0).unwrap();
        plugin.set_gain(0, 1, 1.0).unwrap();

        let input = vec![1.0, 2.0, 3.0, 4.0]; // 2 frames, 2 channels: [L=1,R=2], [L=3,R=4]
        let mut output = vec![0.0; 4];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 2,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Channels should be swapped
        assert_eq!(output[0], 2.0); // Frame 0, Out0 = In1
        assert_eq!(output[1], 1.0); // Frame 0, Out1 = In0
        assert_eq!(output[2], 4.0); // Frame 1, Out0 = In1
        assert_eq!(output[3], 3.0); // Frame 1, Out1 = In0
    }

    #[test]
    fn test_duplicate_to_4_channels() {
        let matrix = vec![
            1.0, 0.0, // Out0 = In0
            0.0, 1.0, // Out1 = In1
            1.0, 0.0, // Out2 = In0
            0.0, 1.0, // Out3 = In1
        ];
        let mut plugin = MatrixPlugin::with_matrix(2, 4, matrix).unwrap();

        let input = vec![1.0, 2.0]; // 1 frame, 2 channels
        let mut output = vec![0.0; 4]; // 1 frame, 4 channels
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        assert_eq!(output[0], 1.0); // Out0 = In0
        assert_eq!(output[1], 2.0); // Out1 = In1
        assert_eq!(output[2], 1.0); // Out2 = In0
        assert_eq!(output[3], 2.0); // Out3 = In1
    }

    #[test]
    fn test_stereo_to_mono_mix() {
        let matrix = vec![0.5, 0.5]; // 2 inputs, 1 output: Out0 = 0.5*In0 + 0.5*In1
        let mut plugin = MatrixPlugin::with_matrix(2, 1, matrix).unwrap();

        let input = vec![2.0, 4.0]; // 1 frame, 2 channels
        let mut output = vec![0.0; 1]; // 1 frame, 1 channel
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        assert_eq!(output[0], 3.0); // 0.5*2.0 + 0.5*4.0 = 3.0
    }

    #[test]
    fn test_parameter_set_get() {
        let mut plugin = MatrixPlugin::new(2, 2);

        // Set gain via parameter system
        plugin
            .set_parameter(ParameterId::from("gain_0_1"), ParameterValue::Float(0.7))
            .unwrap();

        assert_eq!(plugin.get_gain(0, 1), Some(0.7));

        // Get via parameter system
        let value = plugin.get_parameter(&ParameterId::from("gain_0_1"));
        assert_eq!(value, Some(ParameterValue::Float(0.7)));
    }

    #[test]
    fn test_invalid_gain_range() {
        let mut plugin = MatrixPlugin::new(2, 2);
        assert!(plugin.set_gain(0, 0, 1.5).is_err()); // > 1.0
        assert!(plugin.set_gain(0, 0, -0.1).is_err()); // < 0.0
    }

    #[test]
    fn test_invalid_matrix_size() {
        let matrix = vec![1.0, 0.0, 0.0]; // Wrong size for 2x2
        let result = MatrixPlugin::with_matrix(2, 2, matrix);
        assert!(result.is_err());
    }

    #[test]
    fn test_sparse_mapping_basic() {
        // Map physical channels 1,2 to physical channels 15,16 with identity matrix
        let mut plugin = MatrixPlugin::with_sparse_mapping(
            vec![1, 2],   // Read from physical channels 1, 2
            vec![15, 16], // Write to physical channels 15, 16
            vec![
                1.0, 0.0, // PhysOut15 = PhysIn1
                0.0, 1.0, // PhysOut16 = PhysIn2
            ],
        )
        .unwrap();

        // Plugin should report physical channel counts
        assert_eq!(plugin.input_channels(), 3); // max(1,2)+1 = 3
        assert_eq!(plugin.output_channels(), 17); // max(15,16)+1 = 17

        // Create input buffer with 3 physical channels
        let mut input = vec![0.0; 3]; // 1 frame, 3 channels
        input[1] = 10.0; // Physical channel 1
        input[2] = 20.0; // Physical channel 2

        let mut output = vec![0.0; 17]; // 1 frame, 17 channels
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Check that only channels 15 and 16 have values
        for i in 0..17 {
            if i == 15 {
                assert_eq!(output[i], 10.0, "Channel 15 should be 10.0");
            } else if i == 16 {
                assert_eq!(output[i], 20.0, "Channel 16 should be 20.0");
            } else {
                assert_eq!(output[i], 0.0, "Channel {} should be 0.0", i);
            }
        }
    }

    #[test]
    fn test_sparse_mapping_swap() {
        // Map physical channels 1,2 to 15,16 but swap them
        let mut plugin = MatrixPlugin::with_sparse_mapping(
            vec![1, 2],
            vec![15, 16],
            vec![
                0.0, 1.0, // PhysOut15 = PhysIn2
                1.0, 0.0, // PhysOut16 = PhysIn1
            ],
        )
        .unwrap();

        let mut input = vec![0.0; 3]; // 3 physical input channels
        input[1] = 100.0; // Physical channel 1
        input[2] = 200.0; // Physical channel 2

        let mut output = vec![0.0; 17]; // 17 physical output channels
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        assert_eq!(
            output[15], 200.0,
            "Channel 15 should get input from channel 2"
        );
        assert_eq!(
            output[16], 100.0,
            "Channel 16 should get input from channel 1"
        );
    }

    #[test]
    fn test_sparse_mapping_mix() {
        // Mix two channels to one with sparse mapping
        let mut plugin = MatrixPlugin::with_sparse_mapping(
            vec![5, 6],     // Read from channels 5, 6
            vec![10],       // Write to channel 10
            vec![0.5, 0.5], // Out10 = 0.5*In5 + 0.5*In6
        )
        .unwrap();

        assert_eq!(plugin.input_channels(), 7); // max(5,6)+1
        assert_eq!(plugin.output_channels(), 11); // max(10)+1

        let mut input = vec![0.0; 7]; // 7 physical input channels
        input[5] = 10.0;
        input[6] = 20.0;

        let mut output = vec![0.0; 11]; // 11 physical output channels
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        assert_eq!(output[10], 15.0, "Channel 10 should be average of 5 and 6");
        // All other channels should be zero
        for i in 0..11 {
            if i != 10 {
                assert_eq!(output[i], 0.0, "Channel {} should be 0.0", i);
            }
        }
    }

    #[test]
    fn test_sparse_mapping_invalid_empty_maps() {
        let result = MatrixPlugin::with_sparse_mapping(
            vec![], // Empty input map
            vec![15],
            vec![1.0],
        );
        assert!(result.is_err());

        let result = MatrixPlugin::with_sparse_mapping(
            vec![1],
            vec![], // Empty output map
            vec![1.0],
        );
        assert!(result.is_err());
    }
}
