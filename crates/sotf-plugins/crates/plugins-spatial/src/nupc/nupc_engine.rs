use super::partition_level::{PartitionKernel, PartitionLevel};
use super::time_domain_head::TimeDomainHead;
use super::types::plan_partitions;
use rustfft::FftPlanner;
use std::sync::Arc;

pub struct NupcKernel {
    levels: Vec<(Arc<PartitionKernel>, usize)>,
    min_block: usize,
    head_taps: Option<Arc<[f32]>>,
}

impl NupcKernel {
    pub fn new(ir: &[f32], min_block: usize) -> Self {
        let specs = plan_partitions(ir.len(), min_block);
        let mut planner = FftPlanner::new();
        let levels = specs
            .iter()
            .map(|spec| {
                let output_delay = min_block + spec.offset - spec.block_size;
                (PartitionKernel::new(spec, ir, &mut planner), output_delay)
            })
            .collect();
        Self {
            levels,
            min_block,
            head_taps: None,
        }
    }

    pub fn new_with_head(ir: &[f32], min_block: usize, head_taps: usize) -> Self {
        let head_len = head_taps.min(ir.len());
        if head_len == 0 {
            return Self::new(ir, min_block);
        }
        let tail = &ir[head_len..];
        let specs = plan_partitions(tail.len(), min_block.min(head_len));
        let mut planner = FftPlanner::new();
        let levels = specs
            .iter()
            .map(|spec| {
                let absolute_offset = head_len + spec.offset;
                let output_delay = absolute_offset - spec.block_size;
                (PartitionKernel::new(spec, tail, &mut planner), output_delay)
            })
            .collect();
        Self {
            levels,
            min_block,
            head_taps: Some(Arc::from(&ir[..head_len])),
        }
    }

    pub fn instantiate(&self) -> NupcEngine {
        NupcEngine {
            levels: self
                .levels
                .iter()
                .map(|(kernel, delay)| PartitionLevel::from_kernel(Arc::clone(kernel), *delay))
                .collect(),
            min_block: self.min_block,
            td_head: self
                .head_taps
                .as_ref()
                .map(|taps| TimeDomainHead::from_taps(Arc::clone(taps))),
            td_head_len: self.head_taps.as_ref().map_or(0, |taps| taps.len()),
        }
    }
}

/// Non-Uniform Partitioned Convolution engine.
///
/// Uses progressively larger block sizes for efficient long-IR convolution
/// while maintaining low latency from the smallest block size.
/// Optionally includes a time-domain head for zero-latency processing
/// of the first few taps.
pub struct NupcEngine {
    pub(super) levels: Vec<PartitionLevel>,
    pub(super) min_block: usize,
    /// Optional time-domain head for zero-latency first taps
    pub(super) td_head: Option<TimeDomainHead>,
    /// Number of IR samples handled by the time-domain head
    pub(super) td_head_len: usize,
}

impl NupcEngine {
    /// Create a new NUPC engine from an impulse response.
    ///
    /// # Arguments
    /// * `ir` - Impulse response samples (single channel)
    /// * `min_block` - Minimum block size (determines latency)
    pub fn new(ir: &[f32], min_block: usize) -> Self {
        NupcKernel::new(ir, min_block).instantiate()
    }

    /// Create with a time-domain head for zero-latency processing.
    ///
    /// The first `head_taps` samples of the IR are processed in the time domain
    /// (zero latency). The remaining IR is handled by the FFT levels.
    /// The FFT partition plan starts at offset `head_taps` into the IR.
    pub fn new_with_head(ir: &[f32], min_block: usize, head_taps: usize) -> Self {
        NupcKernel::new_with_head(ir, min_block, head_taps).instantiate()
    }

    /// Process a single sample through all partition levels.
    ///
    /// Each level accumulates samples independently at its own block size.
    /// The output is the sum of contributions from all levels.
    pub fn process_sample(&mut self, sample: f32) -> f32 {
        let mut output = 0.0;
        // Time-domain head: zero-latency direct convolution of first taps
        if let Some(ref mut head) = self.td_head {
            output += head.process_sample(sample);
        }
        // FFT levels: handle the remaining IR tail
        for level in &mut self.levels {
            output += level.push_sample(sample);
        }
        output
    }

    /// Returns the number of IR samples handled by the time-domain head (0 if disabled).
    pub fn head_taps(&self) -> usize {
        self.td_head_len
    }

    /// Process a block of samples.
    pub fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        for (i, &sample) in input.iter().enumerate() {
            output[i] = self.process_sample(sample);
        }
    }

    /// Reset all internal state.
    pub fn reset(&mut self) {
        if let Some(ref mut head) = self.td_head {
            head.reset();
        }
        for level in &mut self.levels {
            level.reset();
        }
    }

    /// Get the latency in samples (= min_block).
    pub fn latency_samples(&self) -> usize {
        self.min_block
    }

    pub fn shares_ir_kernel_with(&self, other: &Self) -> bool {
        self.levels.len() == other.levels.len()
            && self
                .levels
                .iter()
                .zip(&other.levels)
                .all(|(a, b)| Arc::ptr_eq(&a.kernel, &b.kernel))
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
