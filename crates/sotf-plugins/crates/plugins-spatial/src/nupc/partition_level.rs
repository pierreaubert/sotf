use super::types::PartitionSpec;
use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use sotf_host::simd::complex_mul_add_simd;
use std::sync::Arc;

/// One partition level handling a specific block size.
pub(super) struct PartitionKernel {
    pub(super) block_size: usize,
    pub(super) fft_size: usize,
    pub(super) ir_partitions: Vec<Vec<Complex<f32>>>,
    pub(super) fft_forward: Arc<dyn Fft<f32>>,
    pub(super) fft_inverse: Arc<dyn Fft<f32>>,
    pub(super) scratch_len: usize,
}

pub(super) struct PartitionLevel {
    pub(super) kernel: Arc<PartitionKernel>,
    /// Frequency Domain delay Line [partition][bin]
    pub(super) fdl: Vec<Vec<Complex<f32>>>,
    pub(super) fdl_head: usize,
    /// Overlap-add accumulator (length = fft_size)
    pub(super) output_accum: Vec<f32>,
    /// Ready output queue (length = block_size, filled after each process_block)
    pub(super) output_queue: Vec<f32>,
    pub(super) output_queue_pos: usize,
    /// Additional delay needed to place this level at its absolute IR offset.
    pub(super) output_delay: Vec<f32>,
    pub(super) output_delay_pos: usize,
    /// Input block accumulator
    pub(super) input_accum: Vec<f32>,
    pub(super) input_fill: usize,
    /// Scratch buffers
    pub(super) fft_scratch: Vec<Complex<f32>>,
    pub(super) fft_spectrum: Vec<Complex<f32>>,
    pub(super) fft_sum: Vec<Complex<f32>>,
}

impl PartitionKernel {
    pub(super) fn new(
        spec: &PartitionSpec,
        ir_data: &[f32],
        planner: &mut FftPlanner<f32>,
    ) -> Arc<Self> {
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

        Arc::new(Self {
            block_size: spec.block_size,
            fft_size: spec.fft_size,
            ir_partitions,
            fft_forward,
            fft_inverse,
            scratch_len,
        })
    }
}

impl PartitionLevel {
    pub(super) fn from_kernel(kernel: Arc<PartitionKernel>, output_delay_samples: usize) -> Self {
        let num_parts = kernel.ir_partitions.len();
        let block_size = kernel.block_size;
        let fft_size = kernel.fft_size;
        let scratch_len = kernel.scratch_len;
        Self {
            kernel,
            fdl: vec![vec![Complex::new(0.0, 0.0); fft_size]; num_parts],
            fdl_head: 0,
            output_accum: vec![0.0; fft_size],
            output_queue: vec![0.0; block_size],
            output_queue_pos: 0,
            output_delay: vec![0.0; output_delay_samples],
            output_delay_pos: 0,
            input_accum: vec![0.0; block_size],
            input_fill: 0,
            fft_scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            fft_spectrum: vec![Complex::new(0.0, 0.0); fft_size],
            fft_sum: vec![Complex::new(0.0, 0.0); fft_size],
        }
    }

    /// Push a single sample and return output sample.
    /// Internally accumulates samples and processes when block is full.
    pub(super) fn push_sample(&mut self, sample: f32) -> f32 {
        // Read from the output queue (contains results from previous block)
        let raw_output = self.output_queue[self.output_queue_pos];
        let output = if self.output_delay.is_empty() {
            raw_output
        } else {
            let delayed = self.output_delay[self.output_delay_pos];
            self.output_delay[self.output_delay_pos] = raw_output;
            self.output_delay_pos += 1;
            if self.output_delay_pos == self.output_delay.len() {
                self.output_delay_pos = 0;
            }
            delayed
        };

        // Accumulate input
        self.input_accum[self.input_fill] = sample;
        self.input_fill += 1;
        self.output_queue_pos += 1;

        if self.input_fill == self.kernel.block_size {
            self.process_block();
            self.input_fill = 0;
            self.output_queue_pos = 0;
        }

        output
    }

    pub(super) fn process_block(&mut self) {
        let b = self.kernel.block_size;
        let num_parts = self.kernel.ir_partitions.len();

        // FFT input block (zero-padded)
        for i in 0..b {
            self.fft_spectrum[i] = Complex::new(self.input_accum[i], 0.0);
        }
        for i in b..self.kernel.fft_size {
            self.fft_spectrum[i] = Complex::new(0.0, 0.0);
        }
        self.kernel
            .fft_forward
            .process_with_scratch(&mut self.fft_spectrum, &mut self.fft_scratch);

        // Push into FDL
        self.fdl_head = if self.fdl_head == 0 {
            num_parts - 1
        } else {
            self.fdl_head - 1
        };
        self.fdl[self.fdl_head].copy_from_slice(&self.fft_spectrum);

        // Convolve: Y = Σ IR[p] ⊙ FDL[p]
        self.fft_sum.fill(Complex::default());
        for p in 0..num_parts {
            let fdl_idx = (self.fdl_head + p) % num_parts;
            complex_mul_add_simd(
                &mut self.fft_sum,
                &self.fdl[fdl_idx],
                &self.kernel.ir_partitions[p],
            );
        }

        // IFFT
        self.kernel
            .fft_inverse
            .process_with_scratch(&mut self.fft_sum, &mut self.fft_scratch);

        // Overlap-add into accumulator
        let inv_n = 1.0 / self.kernel.fft_size as f32;
        for i in 0..self.kernel.fft_size {
            self.output_accum[i] += self.fft_sum[i].re * inv_n;
        }

        // Copy first B samples to output queue (these are the valid output for next block read)
        self.output_queue[..b].copy_from_slice(&self.output_accum[..b]);

        // Shift out consumed samples, keep overlap tail
        self.output_accum.copy_within(b..self.kernel.fft_size, 0);
        self.output_accum[b..].fill(0.0);
    }

    pub(super) fn reset(&mut self) {
        for fdl in &mut self.fdl {
            fdl.fill(Complex::new(0.0, 0.0));
        }
        self.fdl_head = 0;
        self.output_accum.fill(0.0);
        self.output_queue.fill(0.0);
        self.output_queue_pos = 0;
        self.output_delay.fill(0.0);
        self.output_delay_pos = 0;
        self.input_accum.fill(0.0);
        self.input_fill = 0;
    }
}
