//! Render Plan Snapshot Tests
//!
//! Tests layout decisions for all plugins across 10 device profiles.
//! Any layout regression at any screen size produces a JSON diff.
//!
//! Run: cargo test -p sotf-plugins --test render_plan_snapshots
//! Review: cargo insta review

use sotf_host::plugin_params::PluginParamDef;
use sotf_host::render_plan::build_render_plan;

/// Device profiles covering all layout solver breakpoints.
const DEVICE_PROFILES: &[(&str, f32)] = &[
    ("iphone_portrait", 390.0),
    ("iphone_landscape", 844.0),
    ("ipad_portrait", 768.0),
    ("ipad_landscape", 1024.0),
    ("desktop_600", 600.0),
    ("desktop_800", 800.0),
    ("desktop_1000", 1000.0),
    ("desktop_1200", 1200.0),
    ("desktop_1400", 1400.0),
    ("desktop_1800", 1800.0),
];

/// Generates snapshot tests for one plugin across all device profiles.
macro_rules! snapshot_plugin {
    ($test_mod:ident, $params_type:ty) => {
        mod $test_mod {
            use super::*;

            #[test]
            fn all_profiles() {
                for &(profile_name, width) in DEVICE_PROFILES {
                    let plan = build_render_plan::<$params_type>(width);
                    insta::assert_json_snapshot!(
                        format!("{}__{}", stringify!($test_mod), profile_name),
                        plan
                    );
                }
            }
        }
    };
}

// All plugins with PluginParamDef implementations
snapshot_plugin!(ab_compare, sotf_plugin_ab_compare::params::Params);
snapshot_plugin!(aec, sotf_plugin_aec::params::Params);
snapshot_plugin!(ambisonics, sotf_plugin_ambisonics::params::Params);
snapshot_plugin!(band_merge, sotf_plugin_band_merge::params::Params);
snapshot_plugin!(band_split, sotf_plugin_band_split::params::Params);
snapshot_plugin!(beamformer, sotf_plugin_beamformer::params::Params);
snapshot_plugin!(binaural, sotf_plugin_binaural::params::Params);
snapshot_plugin!(channel_mute_solo, sotf_plugin_channel_mute_solo::params::Params);
snapshot_plugin!(convolution, sotf_plugin_convolution::params::Params);
snapshot_plugin!(crossfeed, sotf_plugin_crossfeed::params::Params);
snapshot_plugin!(delay, sotf_plugin_delay::params::Params);
snapshot_plugin!(denoiser, sotf_plugin_denoiser::params::Params);
snapshot_plugin!(dither, sotf_plugin_dither::params::Params);
snapshot_plugin!(downmix, sotf_plugin_downmix::params::Params);

snapshot_plugin!(fletcher_munson, sotf_plugin_fletcher_munson::params::Params);
snapshot_plugin!(gain, sotf_plugin_gain::params::Params);
snapshot_plugin!(gate, sotf_plugin_gate::params::Params);
snapshot_plugin!(limiter, sotf_plugin_limiter::params::Params);
snapshot_plugin!(loudness_compensation, sotf_plugin_loudness_compensation::params::Params);
snapshot_plugin!(matrix, sotf_plugin_matrix::params::Params);
snapshot_plugin!(mono_to_stereo, sotf_plugin_mono_to_stereo::params::Params);
snapshot_plugin!(pnd, sotf_plugin_pnd::params::Params);
snapshot_plugin!(upmixer, sotf_plugin_upmixer::params::Params);
snapshot_plugin!(xtc, sotf_plugin_xtc::params::Params);
