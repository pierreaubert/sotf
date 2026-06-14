use super::partition_level::PartitionLevel;
use super::time_domain_head::TimeDomainHead;
use super::types::plan_partitions;
use rustfft::FftPlanner;

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
        let specs = plan_partitions(ir.len(), min_block);
        let mut planner = FftPlanner::new();

        let levels: Vec<PartitionLevel> = specs
            .iter()
            .map(|spec| PartitionLevel::new(spec, ir, &mut planner))
            .collect();

        Self {
            levels,
            min_block,
            td_head: None,
            td_head_len: 0,
        }
    }

    /// Create with a time-domain head for zero-latency processing.
    ///
    /// The first `head_taps` samples of the IR are processed in the time domain
    /// (zero latency). The remaining IR is handled by the FFT levels.
    /// The FFT partition plan starts at offset `head_taps` into the IR.
    pub fn new_with_head(ir: &[f32], min_block: usize, head_taps: usize) -> Self {
        let head_len = head_taps.min(ir.len());
        if head_len == 0 {
            return Self::new(ir, min_block);
        }

        let td_head = TimeDomainHead::new(ir, head_len);

        // Build FFT levels for the tail (IR starting at head_len)
        let tail = if head_len < ir.len() {
            &ir[head_len..]
        } else {
            &[]
        };
        let specs = plan_partitions(tail.len(), min_block);
        let mut planner = FftPlanner::new();
        let levels: Vec<PartitionLevel> = specs
            .iter()
            .map(|spec| PartitionLevel::new(spec, tail, &mut planner))
            .collect();

        Self {
            levels,
            min_block,
            td_head: Some(td_head),
            td_head_len: head_len,
        }
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
}

impl std::fmt::Debug for NupcEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NupcEngine")
            .field("num_levels", &self.levels.len())
            .field("min_block", &self.min_block)
            .finish()
    }
}
