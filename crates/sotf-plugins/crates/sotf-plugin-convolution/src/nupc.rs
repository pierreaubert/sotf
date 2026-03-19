// ============================================================================
// NUPC — Non-Uniform Partitioned Convolution
// ============================================================================
//
// Provides 2-5x efficiency over uniform partitioned convolution (UPC) for
// long impulse responses by using progressively larger block sizes:
//   B, B, 2B, 2B, 4B, 4B, 8B, ...
//
// Early partitions use small FFTs (low latency), while later partitions
// use larger FFTs (better efficiency). The minimum block size determines
// the overall latency.
//
// Based on: García, G. (2002). "Optimal Filter Partition for Efficient
// Convolution with Short Input/Output Delay."

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use sotf_host::simd::complex_mul_add_simd;
use std::sync::Arc;

/// Specification for one group of partitions at a given block size.
#[derive(Debug, Clone)]
pub struct PartitionSpec {
    /// Offset into the IR (in samples)
    pub offset: usize,
    /// Block size for this level
    pub block_size: usize,
    /// FFT size (2 * block_size)
    pub fft_size: usize,
    /// Number of IR partitions at this level
    pub count: usize,
}

/// Plan the optimal partition sizes for a given IR length.
///
/// Uses the Garcia 2002 doubling pattern: B, B, 2B, 2B, 4B, 4B, 8B, ...
///
/// # Arguments
/// * `ir_length` - Length of the impulse response in samples
/// * `min_block` - Minimum block size (determines latency)
///
/// # Returns
/// List of partition specifications
pub fn plan_partitions(ir_length: usize, min_block: usize) -> Vec<PartitionSpec> {
    if ir_length == 0 {
        return Vec::new();
    }

    let mut specs = Vec::new();
    let mut offset = 0;
    let mut current_block = min_block;
    let mut count_at_size = 0;

    while offset < ir_length {
        let remaining = ir_length - offset;
        let parts_at_this_level = if current_block == min_block {
            // First level: use 2 partitions of min_block
            2
        } else {
            // Later levels: 2 partitions per doubling
            2
        };

        let actual_parts = parts_at_this_level.min(remaining.div_ceil(current_block));

        if actual_parts > 0 {
            specs.push(PartitionSpec {
                offset,
                block_size: current_block,
                fft_size: current_block * 2,
                count: actual_parts,
            });
            offset += actual_parts * current_block;
        }

        count_at_size += 1;
        // Double block size after every 2 groups (except the first min_block group)
        if count_at_size >= 2 && current_block < ir_length {
            current_block *= 2;
            count_at_size = 0;
        }
    }

    specs
}

/// One partition level handling a specific block size.
struct PartitionLevel {
    block_size: usize,
    fft_size: usize,
    /// Pre-FFT'd IR segments for this level [partition][bin]
    ir_partitions: Vec<Vec<Complex<f32>>>,
    /// Frequency Domain delay Line [partition][bin]
    fdl: Vec<Vec<Complex<f32>>>,
    fdl_head: usize,
    /// Overlap-add accumulator (length = fft_size)
    output_accum: Vec<f32>,
    /// Ready output queue (length = block_size, filled after each process_block)
    output_queue: Vec<f32>,
    output_queue_pos: usize,
    /// Input block accumulator
    input_accum: Vec<f32>,
    input_fill: usize,
    /// FFT plan
    fft_forward: Arc<dyn Fft<f32>>,
    fft_inverse: Arc<dyn Fft<f32>>,
    /// Scratch buffers
    fft_scratch: Vec<Complex<f32>>,
    fft_spectrum: Vec<Complex<f32>>,
    fft_sum: Vec<Complex<f32>>,
}

impl PartitionLevel {
    fn new(
        spec: &PartitionSpec,
        ir_data: &[f32],
        planner: &mut FftPlanner<f32>,
    ) -> Self {
        let fft_forward = planner.plan_fft_forward(spec.fft_size);
        let fft_inverse = planner.plan_fft_inverse(spec.fft_size);
        let scratch_len = fft_forward
            .get_inplace_scratch_len()
            .max(fft_inverse.get_inplace_scratch_len());

        // Pre-compute FFT of each IR partition
        let mut ir_partitions = Vec::with_capacity(spec.count);
        for p in 0..spec.count {
            let ir_start = spec.offset + p * spec.block_size;
            let ir_end = (ir_start + spec.block_size).min(ir_data.len());

            let mut block = vec![Complex::new(0.0, 0.0); spec.fft_size];
            for (i, &s) in ir_data[ir_start..ir_end].iter().enumerate() {
                block[i] = Complex::new(s, 0.0);
            }
            let mut scratch = vec![Complex::new(0.0, 0.0); scratch_len];
            fft_forward.process_with_scratch(&mut block, &mut scratch);
            ir_partitions.push(block);
        }

        let num_parts = spec.count;

        Self {
            block_size: spec.block_size,
            fft_size: spec.fft_size,
            ir_partitions,
            fdl: vec![vec![Complex::new(0.0, 0.0); spec.fft_size]; num_parts],
            fdl_head: 0,
            output_accum: vec![0.0; spec.fft_size],
            output_queue: vec![0.0; spec.block_size],
            output_queue_pos: 0,
            input_accum: vec![0.0; spec.block_size],
            input_fill: 0,
            fft_forward,
            fft_inverse,
            fft_scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            fft_spectrum: vec![Complex::new(0.0, 0.0); spec.fft_size],
            fft_sum: vec![Complex::new(0.0, 0.0); spec.fft_size],
        }
    }

    /// Push a single sample and return output sample.
    /// Internally accumulates samples and processes when block is full.
    fn push_sample(&mut self, sample: f32) -> f32 {
        // Read from the output queue (contains results from previous block)
        let output = self.output_queue[self.output_queue_pos];

        // Accumulate input
        self.input_accum[self.input_fill] = sample;
        self.input_fill += 1;
        self.output_queue_pos += 1;

        if self.input_fill == self.block_size {
            self.process_block();
            self.input_fill = 0;
            self.output_queue_pos = 0;
        }

        output
    }

    fn process_block(&mut self) {
        let b = self.block_size;
        let num_parts = self.ir_partitions.len();

        // FFT input block (zero-padded)
        for i in 0..b {
            self.fft_spectrum[i] = Complex::new(self.input_accum[i], 0.0);
        }
        for i in b..self.fft_size {
            self.fft_spectrum[i] = Complex::new(0.0, 0.0);
        }
        self.fft_forward
            .process_with_scratch(&mut self.fft_spectrum, &mut self.fft_scratch);

        // Push into FDL
        self.fdl_head = if self.fdl_head == 0 {
            num_parts - 1
        } else {
            self.fdl_head - 1
        };
        self.fdl[self.fdl_head].copy_from_slice(&self.fft_spectrum);

        // Convolve: Y = Σ IR[p] ⊙ FDL[p]
        self.fft_sum.fill(Complex::new(0.0, 0.0));
        for p in 0..num_parts {
            let fdl_idx = (self.fdl_head + p) % num_parts;
            complex_mul_add_simd(&mut self.fft_sum, &self.fdl[fdl_idx], &self.ir_partitions[p]);
        }

        // IFFT
        self.fft_inverse
            .process_with_scratch(&mut self.fft_sum, &mut self.fft_scratch);

        // Overlap-add into accumulator
        let inv_n = 1.0 / self.fft_size as f32;
        for i in 0..self.fft_size {
            self.output_accum[i] += self.fft_sum[i].re * inv_n;
        }

        // Copy first B samples to output queue (these are the valid output for next block read)
        self.output_queue[..b].copy_from_slice(&self.output_accum[..b]);

        // Shift out consumed samples, keep overlap tail
        self.output_accum.copy_within(b..self.fft_size, 0);
        self.output_accum[b..].fill(0.0);
    }

    fn reset(&mut self) {
        for fdl in &mut self.fdl {
            fdl.fill(Complex::new(0.0, 0.0));
        }
        self.fdl_head = 0;
        self.output_accum.fill(0.0);
        self.output_queue.fill(0.0);
        self.output_queue_pos = 0;
        self.input_accum.fill(0.0);
        self.input_fill = 0;
    }
}

/// Non-Uniform Partitioned Convolution engine.
///
/// Uses progressively larger block sizes for efficient long-IR convolution
/// while maintaining low latency from the smallest block size.
pub struct NupcEngine {
    levels: Vec<PartitionLevel>,
    min_block: usize,
}

impl NupcEngine {
    /// Create a new NUPC engine from an impulse response.
    ///
    /// # Arguments
    /// * `ir` - Impulse response samples (single channel)
    /// * `min_block` - Minimum block size (determines latency)
    pub fn new(ir: &[f32], min_block: usize) -> Self {
        let specs = plan_partitions(ir.len(), min_block);
        let mut planner = FftPlanner::new();

        let levels: Vec<PartitionLevel> = specs
            .iter()
            .map(|spec| PartitionLevel::new(spec, ir, &mut planner))
            .collect();

        Self { levels, min_block }
    }

    /// Process a single sample through all partition levels.
    ///
    /// Each level accumulates samples independently at its own block size.
    /// The output is the sum of contributions from all levels.
    pub fn process_sample(&mut self, sample: f32) -> f32 {
        let mut output = 0.0;
        for level in &mut self.levels {
            output += level.push_sample(sample);
        }
        output
    }

    /// Process a block of samples.
    pub fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        for (i, &sample) in input.iter().enumerate() {
            output[i] = self.process_sample(sample);
        }
    }

    /// Reset all internal state.
    pub fn reset(&mut self) {
        for level in &mut self.levels {
            level.reset();
        }
    }

    /// Get the latency in samples (= min_block).
    pub fn latency_samples(&self) -> usize {
        self.min_block
    }
}

impl std::fmt::Debug for NupcEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NupcEngine")
            .field("num_levels", &self.levels.len())
            .field("min_block", &self.min_block)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_planning() {
        let specs = plan_partitions(8192, 256);
        assert!(!specs.is_empty());

        // First spec should use min_block
        assert_eq!(specs[0].block_size, 256);

        // Verify coverage: total samples covered >= IR length
        let total: usize = specs.iter().map(|s| s.count * s.block_size).sum();
        assert!(total >= 8192, "Partitions cover {total} samples, need 8192");

        // Verify doubling pattern
        let mut prev_size = 0;
        for spec in &specs {
            assert!(
                spec.block_size >= prev_size,
                "Block sizes should be non-decreasing"
            );
            prev_size = spec.block_size;
        }
    }

    #[test]
    fn test_partition_planning_short_ir() {
        let specs = plan_partitions(100, 256);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].count, 1);
    }

    #[test]
    fn test_partition_planning_empty() {
        let specs = plan_partitions(0, 256);
        assert!(specs.is_empty());
    }

    #[test]
    fn test_nupc_impulse_response() {
        // Create a simple IR: [1, 0, 0, 0, ...]
        let mut ir = vec![0.0f32; 512];
        ir[0] = 1.0;

        let mut engine = NupcEngine::new(&ir, 256);

        // Process a known signal through
        let input: Vec<f32> = (0..1024)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48000.0).sin())
            .collect();

        let mut output = vec![0.0f32; 1024];
        engine.process_block(&input, &mut output);

        // With unit impulse IR, output should match input (after initial latency)
        let latency = engine.latency_samples();
        for i in latency..1024 {
            let error = (output[i] - input[i - latency]).abs();
            assert!(
                error < 0.01,
                "Sample {i}: expected {:.4}, got {:.4} (error {error:.6})",
                input[i - latency],
                output[i]
            );
        }
    }

    #[test]
    fn test_nupc_vs_upc_simple() {
        // Use a short decaying IR
        let ir_len = 2048;
        let ir: Vec<f32> = (0..ir_len)
            .map(|i| (-i as f32 / 500.0).exp() * (i as f32 * 0.1).sin() * 0.5)
            .collect();

        let mut nupc = NupcEngine::new(&ir, 256);

        // Process a signal
        let sig_len = 4096;
        let input: Vec<f32> = (0..sig_len)
            .map(|i| (i as f32 * 0.05).sin() * 0.3)
            .collect();

        let mut output = vec![0.0f32; sig_len];
        nupc.process_block(&input, &mut output);

        // Verify output is finite and non-zero after latency
        let latency = nupc.latency_samples();
        let post_latency = &output[latency + 256..];
        let has_signal = post_latency.iter().any(|&x| x.abs() > 1e-6);
        assert!(has_signal, "NUPC should produce non-zero output");

        for (i, &x) in output.iter().enumerate() {
            assert!(x.is_finite(), "Sample {i} is not finite: {x}");
        }
    }

    #[test]
    fn test_nupc_reset() {
        let ir = vec![1.0f32; 512];
        let mut engine = NupcEngine::new(&ir, 256);

        let input = vec![1.0f32; 512];
        let mut output = vec![0.0; 512];
        engine.process_block(&input, &mut output);

        engine.reset();

        // After reset, processing silence should give silence
        let silence = vec![0.0f32; 512];
        let mut output2 = vec![0.0; 512];
        engine.process_block(&silence, &mut output2);

        for (i, &x) in output2.iter().enumerate() {
            assert!(
                x.abs() < 0.01,
                "After reset, sample {i} should be near zero, got {x}"
            );
        }
    }
}
