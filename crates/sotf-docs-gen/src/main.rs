//! Documentation generator for SotF.
//!
//! Reads plugin PARAMS arrays and generates markdown reference pages
//! into `site/src/content/docs/reference/plugins/`.

use sotf_host::param_specs::{ParamSpec, ParamType, UpdateMode};
use std::fmt::Write as FmtWrite;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Plugin registry: (slug, display_name, description, params)
// ---------------------------------------------------------------------------

struct PluginEntry {
    slug: &'static str,
    name: &'static str,
    description: &'static str,
    params: &'static [ParamSpec],
    /// Some plugins have a separate GLOBAL_PARAMS for multiband/EQ config.
    global_params: Option<&'static [ParamSpec]>,
    /// Per-band/filter template params (EQ, multiband compressor/expander).
    band_template: Option<&'static [ParamSpec]>,
}

fn plugin_registry() -> Vec<PluginEntry> {
    use sotf_plugins::param_specs;
    vec![
        PluginEntry {
            slug: "eq",
            name: "Parametric EQ",
            description: "Biquad-based parametric equalizer with peak, shelf, and pass filters. Supports multiple filter bands for precise frequency response shaping.",
            params: &[],
            global_params: Some(param_specs::eq::GLOBAL_PARAMS),
            band_template: Some(param_specs::eq::BAND_TEMPLATE),
        },
        PluginEntry {
            slug: "gain",
            name: "Gain",
            description: "Simple volume control with smooth gain ramping to prevent clicks.",
            params: param_specs::gain::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "compressor",
            name: "Compressor",
            description: "Dynamic range compression with configurable threshold, ratio, attack, release, and makeup gain.",
            params: param_specs::compressor::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "multiband-compressor",
            name: "Multiband Compressor",
            description: "Per-band dynamic range compression with 2-5 frequency bands and independent compressor settings per band.",
            params: param_specs::multiband_compressor::PARAMS,
            global_params: Some(param_specs::multiband_compressor::GLOBAL_PARAMS),
            band_template: None,
        },
        PluginEntry {
            slug: "multiband-expander",
            name: "Multiband Expander",
            description: "Per-band dynamic range expansion with 2-5 frequency bands and independent expander settings per band.",
            params: param_specs::multiband_expander::PARAMS,
            global_params: Some(param_specs::multiband_expander::GLOBAL_PARAMS),
            band_template: None,
        },
        PluginEntry {
            slug: "gate",
            name: "Gate",
            description: "Noise gate that silences audio below a configurable threshold.",
            params: param_specs::gate::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "limiter",
            name: "Limiter",
            description: "Peak limiter to prevent clipping. Ensures output never exceeds the ceiling level.",
            params: param_specs::limiter::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "delay",
            name: "Delay",
            description: "Audio delay with configurable delay time per channel.",
            params: param_specs::delay::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "convolution",
            name: "Convolution",
            description: "FFT-based convolution engine for applying impulse responses (room correction, cabinet simulation, reverb).",
            params: param_specs::convolution::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "matrix",
            name: "Matrix Mixer",
            description: "Channel matrix mixing with per-routing gain control. Route any input channel to any output channel.",
            params: param_specs::matrix::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "channel-mute-solo",
            name: "Channel Mute/Solo",
            description: "Per-channel mute, solo, and dim controls with smooth fade transitions.",
            params: param_specs::channel_mute_solo::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "upmixer",
            name: "Upmixer",
            description: "Stereo to surround upmixing (2ch to 5.0/5.1/7.1) using FFT-based spatial decomposition and VBAP panning.",
            params: param_specs::upmixer::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "downmix",
            name: "Downmix",
            description: "Surround to stereo downmixing with configurable channel contributions.",
            params: param_specs::downmix::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "binaural",
            name: "Binaural Renderer",
            description: "HRTF-based 3D spatial audio rendering. Converts multichannel audio to binaural headphone output using SOFA files.",
            params: param_specs::binaural::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "crossfeed",
            name: "Crossfeed",
            description: "Headphone crossfeed that simulates speaker spacing. Supports Bauer, Meier, and multiband algorithms.",
            params: param_specs::crossfeed::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "xtc",
            name: "Crosstalk Cancellation (XTC)",
            description: "Crosstalk cancellation for speaker playback. Creates a wider stereo image by cancelling inter-speaker interference.",
            params: param_specs::xtc::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "pnd",
            name: "Perceptual Noise Diffusion",
            description: "Perceptual noise diffusion (PND) for improving perceived audio quality through controlled noise shaping.",
            params: param_specs::pnd::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "loudness-compensation",
            name: "Loudness Compensation",
            description: "Equal-loudness contour compensation (Fletcher-Munson). Adjusts frequency response based on playback volume to maintain perceived tonal balance.",
            params: param_specs::loudness_compensation::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "mono-to-stereo",
            name: "Mono to Stereo",
            description: "Converts mono signals to stereo output.",
            params: param_specs::mono_to_stereo::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "denoiser",
            name: "Denoiser",
            description: "Audio denoising using MCRA (Minima Controlled Recursive Averaging) and Wiener filtering.",
            params: param_specs::denoiser::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "ab-compare",
            name: "A/B Compare",
            description: "Side-by-side A/B comparison. Instantly toggle between processed and bypass to evaluate your plugin chain.",
            params: param_specs::ab_compare::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "band-split",
            name: "Band Split",
            description: "Splits the audio signal into separate frequency bands for independent processing.",
            params: param_specs::band_split::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "band-merge",
            name: "Band Merge",
            description: "Merges previously split frequency bands back into a single signal.",
            params: param_specs::band_merge::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "aec",
            name: "Acoustic Echo Cancellation",
            description: "Cancels acoustic echoes from microphone input using reference signal correlation.",
            params: param_specs::aec::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "beamformer",
            name: "Beamformer",
            description: "Microphone array beamforming for directional audio capture.",
            params: param_specs::beamformer::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "de-esser",
            name: "De-Esser",
            description: "Sibilance reduction targeting harsh high-frequency content (s, t, sh sounds).",
            params: param_specs::de_esser::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "dither",
            name: "Dither",
            description: "Adds dither noise for bit-depth reduction, minimizing quantization distortion.",
            params: param_specs::dither::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "dynamic-eq",
            name: "Dynamic EQ",
            description: "Frequency-dependent dynamic equalizer that adjusts filter gain based on signal level.",
            params: param_specs::dynamic_eq::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "linear-phase-eq",
            name: "Linear Phase EQ",
            description: "Zero-phase parametric equalizer using FFT convolution. No phase shift, but adds latency.",
            params: param_specs::linear_phase_eq::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "saturation",
            name: "Saturation",
            description: "Harmonic saturation and soft clipping for adding warmth and character.",
            params: param_specs::saturation::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "spectral-compressor",
            name: "Spectral Compressor",
            description: "Frequency-dependent compression operating in the spectral domain for transparent dynamic control.",
            params: param_specs::spectral_compressor::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "stereo-imager",
            name: "Stereo Imager",
            description: "Controls stereo width from mono to extra-wide, using mid/side processing.",
            params: param_specs::stereo_imager::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "transient-shaper",
            name: "Transient Shaper",
            description: "Shapes attack and sustain characteristics of audio transients.",
            params: param_specs::transient_shaper::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "ambisonics",
            name: "Ambisonics",
            description: "Ambisonics encoding and decoding for immersive spatial audio.",
            params: param_specs::ambisonics::PARAMS,
            global_params: None,
            band_template: None,
        },
    ]
}

// ---------------------------------------------------------------------------
// Markdown generation
// ---------------------------------------------------------------------------

fn format_param_type(param: &ParamSpec) -> String {
    match &param.param_type {
        ParamType::Float { .. } => "Float".to_string(),
        ParamType::Int { .. } => "Int".to_string(),
        ParamType::Bool { .. } => "Bool".to_string(),
        ParamType::Choice { labels, .. } => {
            format!("Choice ({})", labels.join(", "))
        }
        ParamType::FilePath => "File Path".to_string(),
    }
}

fn format_range(param: &ParamSpec) -> String {
    match &param.param_type {
        ParamType::Float { min, max, .. } => format!("{min} .. {max}"),
        ParamType::Int { min, max, .. } => format!("{min} .. {max}"),
        ParamType::Bool { .. } => "On / Off".to_string(),
        ParamType::Choice { labels, .. } => format!("{} options", labels.len()),
        ParamType::FilePath => "-".to_string(),
    }
}

fn format_default(param: &ParamSpec) -> String {
    match &param.param_type {
        ParamType::Float { default, .. } => {
            if *default == default.round() {
                format!("{default:.0}")
            } else {
                format!("{default}")
            }
        }
        ParamType::Int { default, .. } => format!("{default}"),
        ParamType::Bool {
            default,
            true_label,
            false_label,
        } => {
            if *default {
                true_label.to_string()
            } else {
                false_label.to_string()
            }
        }
        ParamType::Choice {
            default_index,
            labels,
        } => labels.get(*default_index).unwrap_or(&"?").to_string(),
        ParamType::FilePath => "-".to_string(),
    }
}

fn generate_params_table(params: &[ParamSpec]) -> String {
    if params.is_empty() {
        return "*No configurable parameters.*\n".to_string();
    }

    let mut md = String::new();

    // Group by group name
    let mut groups: Vec<(&str, Vec<&ParamSpec>)> = Vec::new();
    for p in params {
        if let Some(g) = groups.iter_mut().find(|(name, _)| *name == p.group) {
            g.1.push(p);
        } else {
            groups.push((p.group, vec![p]));
        }
    }

    for (group_name, group_params) in &groups {
        if groups.len() > 1 && !group_name.is_empty() {
            writeln!(md, "\n### {group_name}\n").unwrap();
        }

        writeln!(
            md,
            "| Parameter | Type | Range | Default | Unit | Description |"
        )
        .unwrap();
        writeln!(
            md,
            "|-----------|------|-------|---------|------|-------------|"
        )
        .unwrap();

        for p in group_params {
            let doc = if p.doc.is_empty() { "-" } else { p.doc };
            let unit = if p.unit.is_empty() { "-" } else { p.unit };
            writeln!(
                md,
                "| {} | {} | {} | {} | {} | {} |",
                p.name,
                format_param_type(p),
                format_range(p),
                format_default(p),
                unit,
                doc,
            )
            .unwrap();
        }
    }

    md
}

fn generate_plugin_page(entry: &PluginEntry) -> String {
    let mut md = String::new();

    // Frontmatter
    writeln!(md, "---").unwrap();
    writeln!(md, "title: \"{}\"", entry.name).unwrap();
    writeln!(md, "description: \"{}\"", entry.description).unwrap();
    writeln!(md, "---").unwrap();
    writeln!(md).unwrap();

    // Description
    writeln!(md, "{}", entry.description).unwrap();
    writeln!(md).unwrap();

    // Parameters
    writeln!(md, "## Parameters").unwrap();
    writeln!(md).unwrap();

    if let Some(global) = entry.global_params {
        writeln!(md, "### Global Parameters").unwrap();
        writeln!(md).unwrap();
        md.push_str(&generate_params_table(global));
        writeln!(md).unwrap();
    }

    if let Some(band) = entry.band_template {
        writeln!(md, "### Per-Band Parameters").unwrap();
        writeln!(md).unwrap();
        writeln!(md, "These parameters are repeated for each filter band.").unwrap();
        writeln!(md).unwrap();
        md.push_str(&generate_params_table(band));
        writeln!(md).unwrap();
    }

    if !entry.params.is_empty() {
        if entry.global_params.is_some() || entry.band_template.is_some() {
            writeln!(md, "### Single-Band Parameters").unwrap();
            writeln!(md).unwrap();
        }
        md.push_str(&generate_params_table(entry.params));
        writeln!(md).unwrap();
    }

    // Info box
    let structural_params: Vec<&ParamSpec> = entry
        .params
        .iter()
        .chain(entry.global_params.unwrap_or_default().iter())
        .chain(entry.band_template.unwrap_or_default().iter())
        .filter(|p| p.update_mode == UpdateMode::Structural)
        .collect();

    if !structural_params.is_empty() {
        writeln!(md, ":::note").unwrap();
        write!(md, "**Structural parameters** (").unwrap();
        for (i, p) in structural_params.iter().enumerate() {
            if i > 0 {
                write!(md, ", ").unwrap();
            }
            write!(md, "{}", p.name).unwrap();
        }
        writeln!(
            md,
            ") require rebuilding the plugin when changed. Other parameters update in real-time with zero dropout."
        )
        .unwrap();
        writeln!(md, ":::").unwrap();
    }

    md
}

fn generate_plugin_index(entries: &[PluginEntry]) -> String {
    let mut md = String::new();

    writeln!(md, "---").unwrap();
    writeln!(md, "title: Plugin Reference").unwrap();
    writeln!(
        md,
        "description: Complete reference for all SotF audio plugins."
    )
    .unwrap();
    writeln!(md, "---").unwrap();
    writeln!(md).unwrap();
    writeln!(
        md,
        "SotF includes {} audio processing plugins. Click any plugin for its full parameter reference.",
        entries.len()
    )
    .unwrap();
    writeln!(md).unwrap();

    // Group into categories
    writeln!(md, "## Processing Plugins").unwrap();
    writeln!(md).unwrap();
    writeln!(md, "| Plugin | Description |").unwrap();
    writeln!(md, "|--------|-------------|").unwrap();

    for e in entries {
        writeln!(
            md,
            "| [{}](/reference/plugins/{}/) | {} |",
            e.name, e.slug, e.description
        )
        .unwrap();
    }

    writeln!(md).unwrap();

    md
}

// ---------------------------------------------------------------------------
// File I/O
// ---------------------------------------------------------------------------

fn find_project_root() -> PathBuf {
    let mut dir = std::env::current_dir().expect("current_dir");
    loop {
        if dir.join("Cargo.toml").exists() && dir.join("site").exists() {
            return dir;
        }
        if !dir.pop() {
            panic!("Could not find project root (directory with Cargo.toml + site/)");
        }
    }
}

fn write_if_changed(path: &Path, content: &str) {
    if std::fs::read_to_string(path).is_ok_and(|existing| existing == content) {
        return;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create_dir_all");
    }
    std::fs::write(path, content)
        .unwrap_or_else(|e| panic!("Failed to write {}: {e}", path.display()));
    println!("  wrote {}", path.display());
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let root = find_project_root();
    let docs_dir = root.join("site/src/content/docs");

    println!("Generating plugin reference pages...");

    let registry = plugin_registry();

    // Plugin index page
    let index_md = generate_plugin_index(&registry);
    write_if_changed(&docs_dir.join("reference/plugins/index.md"), &index_md);

    // Individual plugin pages
    for entry in &registry {
        let page_md = generate_plugin_page(entry);
        let filename = format!("{}.md", entry.slug);
        write_if_changed(
            &docs_dir.join("reference/plugins").join(&filename),
            &page_md,
        );
    }

    println!("Done. Generated {} plugin reference pages.", registry.len());
}
