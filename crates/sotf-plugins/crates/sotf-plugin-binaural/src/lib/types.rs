use rustfft::num_complex::Complex;
use sotf_host::sofa::SofaFile;

pub(super) struct BinauralState {
    pub(super) hrtf_filters_freq: Vec<Vec<Complex<f32>>>,
    pub(super) diffuse_field_eq_filter: Option<[Vec<Complex<f32>>; 2]>,
    pub(super) _hrtf_data: Option<SofaFile>,
}
