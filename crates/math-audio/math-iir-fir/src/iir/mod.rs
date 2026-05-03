//! IIR filter implementation (Biquad filters and Parametric EQ)

mod biquad;
mod biquad_bank;
pub mod kautz;
mod peq;
pub mod warped_biquad;

pub use biquad::{Biquad, BiquadCoefficients, BiquadFilterType, Peq};
pub use biquad_bank::BiquadBank;
pub use kautz::{KautzFilter, KautzSection};
pub use peq::{
    FilterRow, compute_peq_response, peq_allpass, peq_butterworth_highpass,
    peq_butterworth_lowpass, peq_butterworth_q, peq_equal, peq_format_apo, peq_format_aupreset,
    peq_format_camilladsp, peq_format_easyeffects, peq_format_pipewire, peq_format_rme_channel,
    peq_format_rme_room, peq_format_roon, peq_format_wavelet, peq_linkwitzriley_highpass,
    peq_linkwitzriley_lowpass, peq_linkwitzriley_q, peq_loudness_gain, peq_preamp_gain,
    peq_preamp_gain_max, peq_print, peq_spl,
};
pub use warped_biquad::{WarpedBiquad, bark_lambda, unwarp_frequency, warp_frequency};
