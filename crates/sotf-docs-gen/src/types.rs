use sotf_host::param_specs::ParamSpec;
use std::path::PathBuf;

pub(super) struct PluginEntry {
    pub(super) slug: &'static str,
    pub(super) name: &'static str,
    pub(super) description: &'static str,
    pub(super) params: &'static [ParamSpec],
    /// Some plugins have a separate GLOBAL_PARAMS for multiband/EQ config.
    pub(super) global_params: Option<&'static [ParamSpec]>,
    /// Per-band/filter template params (EQ, multiband compressor/expander).
    pub(super) band_template: Option<&'static [ParamSpec]>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Args {
    pub(super) root: Option<PathBuf>,
    pub(super) check: bool,
}
