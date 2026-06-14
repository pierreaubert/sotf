use super::PluginFuzzer;
use rand::Rng;
use rand::rngs::StdRng;
use sotf_plugins::{MatrixPlugin, Plugin};

pub(super) struct MatrixFuzzer;

impl PluginFuzzer for MatrixFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        // Generate a random matrix with values in [0, 1]
        // Keep it reasonable: output same channel count as input
        let mut matrix = vec![0.0_f32; channels * channels];

        // Fill with random values
        for val in &mut matrix {
            *val = rng.random_range(0.0..1.0);
        }

        // Optionally make it more identity-like sometimes
        if rng.random_bool(0.3) {
            // 30% chance of mostly-identity matrix
            matrix.fill(0.0);
            for i in 0..channels {
                matrix[i * channels + i] = rng.random_range(0.5..1.0);
            }
        }

        let plugin = MatrixPlugin::with_matrix(channels, channels, matrix.clone())
            .expect("Failed to create MatrixPlugin");

        // Describe the matrix briefly
        let desc = if channels <= 4 {
            format!(
                "{}x{} matrix {:?}",
                channels,
                channels,
                &matrix[..matrix.len().min(8)]
            )
        } else {
            format!("{}x{} matrix (truncated)", channels, channels)
        };

        (Box::new(plugin), desc)
    }
}
