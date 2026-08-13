use crate::{
    CEPS_MEM, Complex, FRAME_SIZE, FREQ_SIZE, NB_BANDS, NB_DELTA_CEPS, NB_FEATURES, PITCH_BUF_SIZE,
    PITCH_FRAME_SIZE, PITCH_MAX_PERIOD, PITCH_MIN_PERIOD, WINDOW_SIZE,
};

/// This is the main entry-point into `nnnoiseless`. It mainly contains the various memory buffers
/// that are used while denoising. As such, this is quite a large struct, and should probably be
/// kept behind some kind of pointer.
///
/// # Example
///
/// ```rust
/// # use nnnoiseless::DenoiseState;
/// // One second of 440Hz sine wave at 48kHz sample rate. Note that the input data consists of
/// // `f32`s, but the values should be in the range of an `i16`.
/// let sine: Vec<_> = (0..48_000)
///     .map(|x| (x as f32 * 440.0 * 2.0 * std::f32::consts::PI / 48_000.0).sin() * i16::MAX as f32)
///     .collect();
/// let mut output = Vec::new();
/// let mut out_buf = [0.0; DenoiseState::FRAME_SIZE];
/// let mut denoise = DenoiseState::new();
/// let mut first = true;
/// for chunk in sine.chunks_exact(DenoiseState::FRAME_SIZE) {
///     denoise.process_frame(&mut out_buf[..], chunk);
///
///     // We throw away the first output, as discussed in the documentation for
///     //`DenoiseState::process_frame`.
///     if !first {
///         output.extend_from_slice(&out_buf[..]);
///     }
///     first = false;
/// }
/// ```
struct DenoiseCore {
    analysis_mem: [f32; FRAME_SIZE],
    /// This is some sort of ring buffer, storing the last bunch of cepstra.
    cepstral_mem: [[f32; crate::NB_BANDS]; crate::CEPS_MEM],
    /// The index pointing to the most recent cepstrum in `cepstral_mem`. The previous cepstra are
    /// at indices mem_id - 1, mem_id - 1, etc (wrapped appropriately).
    mem_id: usize,
    synthesis_mem: [f32; FRAME_SIZE],
    pitch_buf: [f32; crate::PITCH_BUF_SIZE],
    last_gain: f32,
    last_period: usize,
    mem_hp_x: [f32; 2],
    lastg: [f32; crate::NB_BANDS],
}

/// Reused model workspace. Keeping the large FFT, analysis, pitch, and
/// synthesis arrays behind the state allocation avoids tens of KiB of
/// per-frame audio-thread stack growth and repeated zero initialization.
struct DenoiseScratch {
    analysis_window: [f32; WINDOW_SIZE],
    pitch_window: [f32; WINDOW_SIZE],
    pitch_downsample: [f32; PITCH_BUF_SIZE / 2],
    pitch_search_x: [f32; PITCH_FRAME_SIZE / 4],
    pitch_search_y: [f32; PITCH_BUF_SIZE / 4],
    pitch_xcorr: [f32; PITCH_MAX_PERIOD / 2],
    pitch_yy: [f32; PITCH_MAX_PERIOD / 2 + 1],
    pitch_refine: [f32; 3],
    pitch_ac: [f32; 5],
    pitch_lpc: [f32; 4],
    pitch_mem: [f32; 5],
    pitch_lpc2: [f32; 5],
    pitch_copy: [f32; PITCH_BUF_SIZE / 2],
    synthesis_window: [f32; WINDOW_SIZE],
    x_freq: [Complex; FREQ_SIZE],
    pitch_freq: [Complex; WINDOW_SIZE],
    x_time: [f32; FRAME_SIZE],
    ex: [f32; NB_BANDS],
    ep: [f32; NB_BANDS],
    exp: [f32; NB_BANDS],
    features: [f32; NB_FEATURES],
    gains: [f32; NB_BANDS],
    interpolated_gains: [f32; FREQ_SIZE],
    vad: [f32; 1],
    ly: [f32; NB_BANDS],
    tmp: [f32; NB_BANDS],
    pitch_filter_r: [f32; NB_BANDS],
    pitch_filter_rf: [f32; FREQ_SIZE],
    pitch_filter_energy: [f32; NB_BANDS],
    pitch_filter_norm: [f32; NB_BANDS],
    pitch_filter_normf: [f32; FREQ_SIZE],
    fft_input: [Complex; WINDOW_SIZE],
    fft_output: [Complex; WINDOW_SIZE],
}

impl DenoiseScratch {
    fn new() -> Self {
        Self {
            analysis_window: [0.0; WINDOW_SIZE],
            pitch_window: [0.0; WINDOW_SIZE],
            pitch_downsample: [0.0; PITCH_BUF_SIZE / 2],
            pitch_search_x: [0.0; PITCH_FRAME_SIZE / 4],
            pitch_search_y: [0.0; PITCH_BUF_SIZE / 4],
            pitch_xcorr: [0.0; PITCH_MAX_PERIOD / 2],
            pitch_yy: [0.0; PITCH_MAX_PERIOD / 2 + 1],
            pitch_refine: [0.0; 3],
            pitch_ac: [0.0; 5],
            pitch_lpc: [0.0; 4],
            pitch_mem: [0.0; 5],
            pitch_lpc2: [0.0; 5],
            pitch_copy: [0.0; PITCH_BUF_SIZE / 2],
            synthesis_window: [0.0; WINDOW_SIZE],
            x_freq: [Complex::new(0.0, 0.0); FREQ_SIZE],
            pitch_freq: [Complex::new(0.0, 0.0); WINDOW_SIZE],
            x_time: [0.0; FRAME_SIZE],
            ex: [0.0; NB_BANDS],
            ep: [0.0; NB_BANDS],
            exp: [0.0; NB_BANDS],
            features: [0.0; NB_FEATURES],
            gains: [0.0; NB_BANDS],
            interpolated_gains: [1.0; FREQ_SIZE],
            vad: [0.0; 1],
            ly: [0.0; NB_BANDS],
            tmp: [0.0; NB_BANDS],
            pitch_filter_r: [0.0; NB_BANDS],
            pitch_filter_rf: [0.0; FREQ_SIZE],
            pitch_filter_energy: [0.0; NB_BANDS],
            pitch_filter_norm: [0.0; NB_BANDS],
            pitch_filter_normf: [0.0; FREQ_SIZE],
            fft_input: [Complex::new(0.0, 0.0); WINDOW_SIZE],
            fft_output: [Complex::new(0.0, 0.0); WINDOW_SIZE],
        }
    }
}

pub struct DenoiseState {
    core: DenoiseCore,
    rnn: crate::rnn::RnnState,
    scratch: Box<DenoiseScratch>,
}

impl DenoiseState {
    /// A `DenoiseState` processes this many samples at a time.
    pub const FRAME_SIZE: usize = FRAME_SIZE;

    /// Creates a new `DenoiseState`.
    pub fn new() -> Box<DenoiseState> {
        Box::new(DenoiseState {
            core: DenoiseCore {
                analysis_mem: [0.0; FRAME_SIZE],
                cepstral_mem: [[0.0; NB_BANDS]; CEPS_MEM],
                mem_id: 0,
                synthesis_mem: [0.0; FRAME_SIZE],
                pitch_buf: [0.0; PITCH_BUF_SIZE],
                last_gain: 0.0,
                last_period: 0,
                mem_hp_x: [0.0; 2],
                lastg: [0.0; NB_BANDS],
            },
            rnn: crate::rnn::RnnState::new(),
            scratch: Box::new(DenoiseScratch::new()),
        })
    }

    /// Processes a chunk of samples.
    ///
    /// Both `output` and `input` should be slices of length `DenoiseState::FRAME_SIZE`.
    ///
    /// The current output of `process_frame` depends on the current input, but also on the
    /// preceding inputs. Because of this, you might prefer to discard the very first output; it
    /// will contain some fade-in artifacts.
    pub fn process_frame(&mut self, output: &mut [f32], input: &[f32]) {
        process_frame(self, output, input);
    }

    /// Resets all internal state to zero without heap allocation.
    pub fn reset(&mut self) {
        self.core.analysis_mem.fill(0.0);
        self.core.cepstral_mem = [[0.0; crate::NB_BANDS]; crate::CEPS_MEM];
        self.core.mem_id = 0;
        self.core.synthesis_mem.fill(0.0);
        self.core.pitch_buf.fill(0.0);
        self.core.last_gain = 0.0;
        self.core.last_period = 0;
        self.core.mem_hp_x.fill(0.0);
        self.core.lastg.fill(0.0);
        self.rnn.reset();
    }
}

fn frame_analysis(core: &mut DenoiseCore, scratch: &mut DenoiseScratch) {
    let buf = &mut scratch.analysis_window;
    for i in 0..FRAME_SIZE {
        buf[i] = core.analysis_mem[i];
    }
    for i in 0..crate::FRAME_SIZE {
        buf[i + crate::FRAME_SIZE] = scratch.x_time[i];
        core.analysis_mem[i] = scratch.x_time[i];
    }
    crate::apply_window(&mut buf[..]);
    crate::forward_transform(
        &mut scratch.x_freq,
        &buf[..],
        &mut scratch.fft_input,
        &mut scratch.fft_output,
    );
    crate::compute_band_corr(&mut scratch.ex, &scratch.x_freq, &scratch.x_freq);
}

fn compute_frame_features(core: &mut DenoiseCore, scratch: &mut DenoiseScratch) -> usize {
    frame_analysis(core, scratch);
    for i in 0..(PITCH_BUF_SIZE - FRAME_SIZE) {
        core.pitch_buf[i] = core.pitch_buf[i + FRAME_SIZE];
    }
    for i in 0..FRAME_SIZE {
        core.pitch_buf[PITCH_BUF_SIZE - FRAME_SIZE + i] = scratch.x_time[i];
    }

    crate::pitch_downsample(
        &core.pitch_buf[..],
        &mut scratch.pitch_downsample,
        &mut scratch.pitch_ac,
        &mut scratch.pitch_lpc,
        &mut scratch.pitch_mem,
        &mut scratch.pitch_lpc2,
        &mut scratch.pitch_copy,
    );
    let pitch_idx = crate::pitch_search(
        &scratch.pitch_downsample[(PITCH_MAX_PERIOD / 2)..],
        &scratch.pitch_downsample,
        PITCH_FRAME_SIZE,
        PITCH_MAX_PERIOD - 3 * PITCH_MIN_PERIOD,
        &mut scratch.pitch_search_x,
        &mut scratch.pitch_search_y,
        &mut scratch.pitch_xcorr,
    );
    let pitch_idx = PITCH_MAX_PERIOD - pitch_idx;

    let (pitch_idx, gain) = crate::remove_doubling(
        &scratch.pitch_downsample[..],
        PITCH_MAX_PERIOD,
        PITCH_MIN_PERIOD,
        PITCH_FRAME_SIZE,
        pitch_idx,
        core.last_period,
        core.last_gain,
        &mut scratch.pitch_yy,
        &mut scratch.pitch_refine,
    );
    core.last_period = pitch_idx;
    core.last_gain = gain;

    for i in 0..WINDOW_SIZE {
        scratch.pitch_window[i] = core.pitch_buf[PITCH_BUF_SIZE - WINDOW_SIZE - pitch_idx + i];
    }
    crate::apply_window(&mut scratch.pitch_window[..]);
    crate::forward_transform(
        &mut scratch.pitch_freq,
        &scratch.pitch_window,
        &mut scratch.fft_input,
        &mut scratch.fft_output,
    );
    crate::compute_band_corr(&mut scratch.ep, &scratch.pitch_freq, &scratch.pitch_freq);
    crate::compute_band_corr(&mut scratch.exp, &scratch.x_freq, &scratch.pitch_freq);
    for i in 0..NB_BANDS {
        scratch.exp[i] /= (0.001 + scratch.ex[i] * scratch.ep[i]).sqrt();
    }
    crate::dct(&mut scratch.tmp[..], &scratch.exp);
    for i in 0..NB_DELTA_CEPS {
        scratch.features[NB_BANDS + 2 * NB_DELTA_CEPS + i] = scratch.tmp[i];
    }

    scratch.features[NB_BANDS + 2 * NB_DELTA_CEPS] -= 1.3;
    scratch.features[NB_BANDS + 2 * NB_DELTA_CEPS + 1] -= 0.9;
    scratch.features[NB_BANDS + 3 * NB_DELTA_CEPS] = 0.01 * (pitch_idx as f32 - 300.0);
    let mut log_max = -2.0;
    let mut follow = -2.0;
    let mut e = 0.0;
    for i in 0..NB_BANDS {
        scratch.ly[i] = (1e-2 + scratch.ex[i])
            .log10()
            .max(log_max - 7.0)
            .max(follow - 1.5);
        log_max = log_max.max(scratch.ly[i]);
        follow = (follow - 1.5).max(scratch.ly[i]);
        e += scratch.ex[i];
    }

    if e < 0.04 {
        /* If there's no audio, avoid messing up the state. */
        scratch.features.fill(0.0);
        return 1;
    }
    crate::dct(&mut scratch.features, &scratch.ly[..]);
    scratch.features[0] -= 12.0;
    scratch.features[1] -= 4.0;
    let ceps_0_idx = core.mem_id;
    let ceps_1_idx = if core.mem_id < 1 {
        CEPS_MEM + core.mem_id - 1
    } else {
        core.mem_id - 1
    };
    let ceps_2_idx = if core.mem_id < 2 {
        CEPS_MEM + core.mem_id - 2
    } else {
        core.mem_id - 2
    };

    for i in 0..NB_BANDS {
        core.cepstral_mem[ceps_0_idx][i] = scratch.features[i];
    }
    core.mem_id += 1;

    let ceps_0 = &core.cepstral_mem[ceps_0_idx];
    let ceps_1 = &core.cepstral_mem[ceps_1_idx];
    let ceps_2 = &core.cepstral_mem[ceps_2_idx];
    for i in 0..NB_DELTA_CEPS {
        scratch.features[i] = ceps_0[i] + ceps_1[i] + ceps_2[i];
        scratch.features[NB_BANDS + i] = ceps_0[i] - ceps_2[i];
        scratch.features[NB_BANDS + NB_DELTA_CEPS + i] = ceps_0[i] - 2.0 * ceps_1[i] + ceps_2[i];
    }

    /* Spectral variability features. */
    let mut spec_variability = 0.0;
    if core.mem_id == CEPS_MEM {
        core.mem_id = 0;
    }
    for i in 0..CEPS_MEM {
        let mut min_dist = 1e15f32;
        for j in 0..CEPS_MEM {
            let mut dist = 0.0;
            for k in 0..NB_BANDS {
                let tmp = core.cepstral_mem[i][k] - core.cepstral_mem[j][k];
                dist += tmp * tmp;
            }
            if j != i {
                min_dist = min_dist.min(dist);
            }
        }
        spec_variability += min_dist;
    }

    scratch.features[NB_BANDS + 3 * NB_DELTA_CEPS + 1] = spec_variability / CEPS_MEM as f32 - 2.1;

    0
}

fn frame_synthesis(core: &mut DenoiseCore, scratch: &mut DenoiseScratch, out: &mut [f32]) {
    crate::inverse_transform(
        &mut scratch.synthesis_window[..],
        &scratch.x_freq,
        &mut scratch.fft_input,
        &mut scratch.fft_output,
    );
    crate::apply_window(&mut scratch.synthesis_window[..]);
    for i in 0..FRAME_SIZE {
        out[i] = scratch.synthesis_window[i] + core.synthesis_mem[i];
        core.synthesis_mem[i] = scratch.synthesis_window[FRAME_SIZE + i];
    }
}

fn biquad(y: &mut [f32], mem: &mut [f32], x: &[f32], b: &[f32], a: &[f32]) {
    for i in 0..x.len() {
        let xi = x[i] as f64;
        let yi = (x[i] + mem[0]) as f64;
        mem[0] = (mem[1] as f64 + (b[0] as f64 * xi - a[0] as f64 * yi)) as f32;
        mem[1] = (b[1] as f64 * xi - a[1] as f64 * yi) as f32;
        y[i] = yi as f32;
    }
}

fn pitch_filter(scratch: &mut DenoiseScratch) {
    for i in 0..NB_BANDS {
        scratch.pitch_filter_r[i] = if scratch.exp[i] > scratch.gains[i] {
            1.0
        } else {
            let exp_sq = scratch.exp[i] * scratch.exp[i];
            let g_sq = scratch.gains[i] * scratch.gains[i];
            exp_sq * (1.0 - g_sq) / (0.001 + g_sq * (1.0 - exp_sq))
        };
        scratch.pitch_filter_r[i] = 1.0_f32.min(0.0_f32.max(scratch.pitch_filter_r[i])).sqrt();
        scratch.pitch_filter_r[i] *= (scratch.ex[i] / (1e-8 + scratch.ep[i])).sqrt();
    }
    crate::interp_band_gain(&mut scratch.pitch_filter_rf, &scratch.pitch_filter_r);
    for i in 0..FREQ_SIZE {
        scratch.x_freq[i] += scratch.pitch_filter_rf[i] * scratch.pitch_freq[i];
    }

    crate::compute_band_corr(
        &mut scratch.pitch_filter_energy,
        &scratch.x_freq,
        &scratch.x_freq,
    );
    for i in 0..NB_BANDS {
        scratch.pitch_filter_norm[i] =
            (scratch.ex[i] / (1e-8 + scratch.pitch_filter_energy[i])).sqrt();
    }
    crate::interp_band_gain(&mut scratch.pitch_filter_normf, &scratch.pitch_filter_norm);
    for i in 0..FREQ_SIZE {
        scratch.x_freq[i] *= scratch.pitch_filter_normf[i];
    }
}

fn process_frame(state: &mut DenoiseState, output: &mut [f32], input: &[f32]) -> f32 {
    let a_hp = [-1.99599, 0.99600];
    let b_hp = [-2.0, 1.0];
    let DenoiseState { core, rnn, scratch } = state;
    let scratch = scratch.as_mut();
    scratch.interpolated_gains.fill(1.0);
    scratch.vad[0] = 0.0;

    biquad(
        &mut scratch.x_time[..],
        &mut core.mem_hp_x[..],
        input,
        &b_hp[..],
        &a_hp[..],
    );
    let silence = compute_frame_features(core, scratch);
    if silence == 0 {
        crate::rnn::compute_rnn(rnn, &mut scratch.gains, &mut scratch.vad, &scratch.features);
        pitch_filter(scratch);
        for i in 0..NB_BANDS {
            scratch.gains[i] = scratch.gains[i].max(0.6 * core.lastg[i]);
            core.lastg[i] = scratch.gains[i];
        }
        crate::interp_band_gain(&mut scratch.interpolated_gains, &scratch.gains);
        for i in 0..FREQ_SIZE {
            scratch.x_freq[i] *= scratch.interpolated_gains[i];
        }
    }

    frame_synthesis(core, scratch, output);
    scratch.vad[0]
}
