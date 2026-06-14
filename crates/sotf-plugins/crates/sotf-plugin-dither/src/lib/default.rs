use crate::params::PARAMS as DT;
use sotf_host::param_specs::find_by_key as pk;

pub(super) fn default_bit_depth() -> usize {
    pk(DT, "bit_depth").default_usize()
}

pub(super) fn default_noise_shaping() -> bool {
    pk(DT, "noise_shaping").default_bool()
}

pub(super) fn default_dither_type() -> usize {
    pk(DT, "dither_type").default_usize()
}
