
#[cfg(test)]
mod tests {
    use sotf_plugins::{MatrixPlugin, Plugin, ProcessContext, ChannelState};

    #[test]
    fn test_matrix_plugin_mute_logic() {
        // 1. Create 2x2 identity matrix plugin
        let matrix = vec![1.0, 0.0, 0.0, 1.0];
        let mut plugin = MatrixPlugin::with_matrix(2, 2, matrix).unwrap();

        // 2. Process audio WITHOUT mute (Baseline)
        let input = vec![1.0, 1.0]; // 1 frame, both channels 1.0
        let mut output = vec![0.0; 2];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1,
        };

        plugin.process(&input, &mut output, &context).unwrap();
        assert_eq!(output[0], 1.0, "Baseline Ch0 should be 1.0");
        assert_eq!(output[1], 1.0, "Baseline Ch1 should be 1.0");

        // 3. Apply Mute to Channel 0 (Left)
        let states = vec![
            ChannelState { muted: true, soloed: false, dimmed: false },
            ChannelState { muted: false, soloed: false, dimmed: false },
        ];
        plugin = plugin.with_channel_states(states);

        // 4. Process audio WITH mute
        let mut output_muted = vec![0.0; 2];
        plugin.process(&input, &mut output_muted, &context).unwrap();

        assert_eq!(output_muted[0], 0.0, "Ch0 should be muted (0.0)");
        assert_eq!(output_muted[1], 1.0, "Ch1 should be unmuted (1.0)");
    }
    
    #[test]
    fn test_matrix_plugin_dim_logic() {
        // 1. Create 2x2 identity matrix plugin
        let matrix = vec![1.0, 0.0, 0.0, 1.0];
        let mut plugin = MatrixPlugin::with_matrix(2, 2, matrix).unwrap();
        
        let input = vec![1.0, 1.0];
        let context = ProcessContext {
            sample_rate: 48000,
            num_frames: 1,
        };

        // 2. Apply Dim to Channel 0
        let states = vec![
            ChannelState { muted: false, soloed: false, dimmed: true },
            ChannelState { muted: false, soloed: false, dimmed: false },
        ];
        plugin = plugin.with_channel_states(states);

        // 3. Process
        let mut output_dimmed = vec![0.0; 2];
        plugin.process(&input, &mut output_dimmed, &context).unwrap();

        // 4. Verify Dim (0.1 factor)
        assert!((output_dimmed[0] - 0.1f32).abs() < 0.0001, "Ch0 should be dimmed to 0.1, got {}", output_dimmed[0]);
        assert_eq!(output_dimmed[1], 1.0, "Ch1 should be unchanged");
    }
}
