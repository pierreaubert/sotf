use math_audio_dsp::stft::{RealFftProcessor, generate_hann_window};
use math_audio_dsp::tonal_transient::TonalTransientSeparator;
use rustfft::num_complex::Complex;

/// All STFT buffers needed for spectral processing.
/// Pre-allocated to avoid hot-path allocation.
pub(super) struct StftState {
    pub(super) fft_size: usize,
    pub(super) hop_size: usize,
    pub(super) num_bins: usize,

    /// Per-channel FFT processor (forward + inverse)
    pub(super) fft_processors: Vec<RealFftProcessor>,

    /// Hann analysis window (length = fft_size)
    pub(super) analysis_window: Vec<f32>,

    /// Combined COLA normalization + 1/fft_size scale factor.
    /// For 75% overlap dual-windowing periodic Hann: 1/(1.5 * N)
    /// (4 overlapping Hann² windows sum to exactly 1.5 at all positions)
    pub(super) output_scale: f32,

    // --- Input staging ---
    /// Per-channel linear input buffer [channels][fft_size]
    pub(super) input_buffers: Vec<Vec<f32>>,
    /// How many valid samples are in the tail of each input_buffer
    pub(super) input_fill: usize,

    // --- Per-channel, per-bin envelope state ---
    /// [channels][num_bins] — smoothed gain reduction in dB (0 = no compression)
    pub(super) bin_envelopes: Vec<Vec<f32>>,

    // --- OLA output accumulator ---
    /// Flat interleaved ring buffer: [ch0_f0, ch1_f0, ch0_f1, ...]
    pub(super) output_accumulator: Vec<f32>,
    pub(super) output_accumulator_mask: usize,
    pub(super) output_accumulator_fill: usize,
    pub(super) next_add_position: usize,
    pub(super) output_read_position: usize,
    pub(super) latency_filled: usize,
    /// Dry path delayed by the reported STFT latency for phase-aligned mixing.
    pub(super) dry_delay_buf: Vec<f32>,
    /// Interleaved frame offset into `dry_delay_buf`.
    pub(super) dry_delay_pos: usize,

    // --- Temporary working buffers ---
    /// Frequency-domain scratch [num_bins]
    pub(super) freq_scratch: Vec<Complex<f32>>,
    /// Scratch for envelope values after median + smoothing [num_bins]
    pub(super) gains_scratch: Vec<f32>,

    // --- Phase 4A: Tonal/Transient separation ---
    /// Per-channel tonal/transient separator
    pub(super) tonal_transient: Vec<TonalTransientSeparator>,
    /// Scratch for magnitudes [num_bins]
    pub(super) magnitudes_scratch: Vec<f32>,
    /// Scratch for tonal mask [num_bins]
    pub(super) tonal_mask: Vec<f32>,
    /// Scratch for transient mask [num_bins]
    pub(super) transient_mask: Vec<f32>,
    /// Per-channel long-term spectral average for adaptive threshold [channels][num_bins]
    pub(super) adaptive_avg: Vec<Vec<f32>>,
}

impl StftState {
    pub(super) fn new(fft_size: usize, channels: usize) -> Self {
        let hop_size = fft_size / 4; // 75% overlap
        let num_bins = fft_size / 2 + 1;

        let fft_processors: Vec<RealFftProcessor> = (0..channels)
            .map(|_| RealFftProcessor::new_bidirectional(fft_size))
            .collect();

        let analysis_window = generate_hann_window(fft_size);

        // 75% overlap, dual periodic Hann (Hann²): COLA sum = 1.5 (exact)
        let output_scale = 1.0 / (fft_size as f32 * 1.5);

        let bin_envelopes: Vec<Vec<f32>> = (0..channels).map(|_| vec![0.0; num_bins]).collect();

        let output_accumulator_frames = (fft_size * 4).next_power_of_two();
        let output_accumulator = vec![0.0f32; output_accumulator_frames * channels];

        Self {
            fft_size,
            hop_size,
            num_bins,
            fft_processors,
            analysis_window,
            output_scale,
            input_buffers: vec![vec![0.0f32; fft_size]; channels],
            input_fill: 0,
            bin_envelopes,
            output_accumulator,
            output_accumulator_mask: output_accumulator_frames - 1,
            output_accumulator_fill: 0,
            next_add_position: 0,
            output_read_position: 0,
            latency_filled: 0,
            dry_delay_buf: vec![0.0; fft_size * channels],
            dry_delay_pos: 0,
            freq_scratch: vec![Complex::new(0.0, 0.0); num_bins],
            gains_scratch: vec![0.0; num_bins],
            // Phase 4A: Tonal/Transient
            tonal_transient: (0..channels)
                .map(|_| TonalTransientSeparator::new(num_bins, 7, 7))
                .collect(),
            magnitudes_scratch: vec![0.0; num_bins],
            tonal_mask: vec![0.0; num_bins],
            transient_mask: vec![0.0; num_bins],
            adaptive_avg: vec![vec![-20.0; num_bins]; channels], // init near typical threshold to avoid startup transient
        }
    }

    pub(super) fn reset(&mut self) {
        for buf in &mut self.input_buffers {
            buf.fill(0.0);
        }
        self.input_fill = 0;
        for env in &mut self.bin_envelopes {
            env.fill(0.0);
        }
        for tt in &mut self.tonal_transient {
            tt.reset();
        }
        for avg in &mut self.adaptive_avg {
            avg.fill(-20.0);
        }
        self.output_accumulator.fill(0.0);
        self.output_accumulator_fill = 0;
        self.next_add_position = 0;
        self.output_read_position = 0;
        self.latency_filled = 0;
        self.dry_delay_buf.fill(0.0);
        self.dry_delay_pos = 0;
    }
}
