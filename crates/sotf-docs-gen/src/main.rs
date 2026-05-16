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
            slug: "expander",
            name: "Expander",
            description: "Dynamic range expansion with configurable threshold, ratio, attack, release, and range. Opens up dynamics below the threshold.",
            params: param_specs::expander::PARAMS,
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
        PluginEntry {
            slug: "spectrum-analyzer",
            name: "Spectrum Analyzer",
            description: "FFT-based spectrum analysis with configurable bin count, frequency range, smoothing, and tilt correction.",
            params: param_specs::spectrum::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "aae",
            name: "Active Acoustic Enhancement",
            description: "Active acoustic enhancement using psychoacoustic processing to improve perceived clarity, presence, and depth.",
            params: param_specs::aae::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "declick",
            name: "Declick",
            description: "Removes clicks, pops, and short transient defects from audio (vinyl restoration, recording artifacts).",
            params: param_specs::declick::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "hiss-reducer",
            name: "Hiss Reducer",
            description: "Reduces high-frequency hiss and tape noise while preserving program content.",
            params: param_specs::hiss_reducer::PARAMS,
            global_params: None,
            band_template: None,
        },
        PluginEntry {
            slug: "speech-denoiser",
            name: "Speech Denoiser",
            description: "Neural speech-focused denoiser (RNNoise-derived) optimized for voice and dialogue cleanup.",
            params: param_specs::speech_denoiser::PARAMS,
            global_params: None,
            band_template: None,
        },
    ]
}

// ---------------------------------------------------------------------------
// Escaping helpers
// ---------------------------------------------------------------------------

/// Escape a string so it is safe to embed inside a single markdown table cell.
///
/// Markdown pipe tables use `|` as a column separator and treat each row as a
/// single line. Backslashes also need escaping so they do not consume the
/// following character. Newlines and carriage returns are replaced with `<br>`
/// so the cell continues to render on a single row.
fn md_cell(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '|' => out.push_str("\\|"),
            '\n' | '\r' => out.push_str("<br>"),
            _ => out.push(ch),
        }
    }
    out
}

/// Escape a string for use inside a YAML double-quoted scalar.
///
/// We restrict ourselves to single-line strings: any newline or carriage
/// return is replaced with a space so the value stays a single-line scalar.
/// Backslashes and double quotes get backslash-escaped; control characters
/// (other than the newline replacement above) are also stripped because they
/// are not allowed in double-quoted YAML scalars without unicode escapes,
/// which we deliberately avoid here.
fn yaml_double_quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' => out.push(' '),
            // Strip other control characters to keep the scalar valid.
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out
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
        } => labels
            .get(*default_index)
            .unwrap_or_else(|| {
                panic!(
                    "Choice default_index {} out of range (labels.len() = {})",
                    default_index,
                    labels.len()
                )
            })
            .to_string(),
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
                md_cell(p.name),
                md_cell(&format_param_type(p)),
                md_cell(&format_range(p)),
                md_cell(&format_default(p)),
                md_cell(unit),
                md_cell(doc),
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
    writeln!(md, "title: \"{}\"", yaml_double_quoted(entry.name)).unwrap();
    writeln!(
        md,
        "description: \"{}\"",
        yaml_double_quoted(entry.description)
    )
    .unwrap();
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
            md_cell(e.name),
            e.slug,
            md_cell(e.description),
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_host::param_specs::ParamCategory;

    fn make_float_spec(
        name: &'static str,
        doc: &'static str,
        unit: &'static str,
    ) -> ParamSpec {
        ParamSpec {
            name,
            engine_key: "test_key",
            param_type: ParamType::Float {
                default: 0.5,
                min: 0.0,
                max: 1.0,
                step: 0.01,
            },
            unit,
            group: "",
            update_mode: UpdateMode::Realtime,
            display_scale: 1.0,
            category: ParamCategory::Primary,
            doc,
        }
    }

    #[test]
    fn md_cell_escapes_pipes_backslashes_and_newlines() {
        assert_eq!(md_cell("a|b"), "a\\|b");
        assert_eq!(md_cell("a\\b"), "a\\\\b");
        assert_eq!(md_cell("line1\nline2"), "line1<br>line2");
        assert_eq!(md_cell("a\rb"), "a<br>b");
        assert_eq!(md_cell("a|b\\c\nd"), "a\\|b\\\\c<br>d");
        assert_eq!(md_cell("hello world"), "hello world");
    }

    #[test]
    fn yaml_double_quoted_escapes_quotes_and_backslashes() {
        assert_eq!(yaml_double_quoted("a\"b"), "a\\\"b");
        assert_eq!(yaml_double_quoted("a\\b"), "a\\\\b");
        assert_eq!(yaml_double_quoted("a\nb"), "a b");
        assert_eq!(yaml_double_quoted("a\rb"), "a b");
        assert_eq!(
            yaml_double_quoted("Adds \"warmth\"\nto signal"),
            "Adds \\\"warmth\\\" to signal"
        );
    }

    /// Pipe-table rows must be single-line and have the expected number of
    /// non-escaped column separators (one more than the cell count).
    fn assert_single_row(row: &str, expected_cells: usize) {
        assert!(!row.contains('\n'), "row contains a raw newline: {row:?}");
        assert!(row.starts_with('|'), "row does not start with '|': {row:?}");
        assert!(row.ends_with('|'), "row does not end with '|': {row:?}");
        let bytes = row.as_bytes();
        let separator_count = bytes
            .iter()
            .enumerate()
            .filter(|&(i, &c)| c == b'|' && (i == 0 || bytes[i - 1] != b'\\'))
            .count();
        assert_eq!(
            separator_count,
            expected_cells + 1,
            "expected {} cells in row but found {} separators: {row:?}",
            expected_cells,
            separator_count.saturating_sub(1),
        );
    }

    #[test]
    fn params_table_round_trips_pipes_quotes_and_newlines() {
        let evil_name = "Gain | \"main\"";
        let evil_doc = "Sets the \"main\" gain.\nUse with |care|.\\nope";
        let evil_unit = "dB|FS";
        let spec = make_float_spec(evil_name, evil_doc, evil_unit);

        let table = generate_params_table(std::slice::from_ref(&spec));

        let lines: Vec<&str> = table.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 3, "expected 3 lines, got: {table}");
        assert_single_row(lines[0], 6); // header
        // lines[1] is the |---|---| separator row — skip strict check.
        assert_single_row(lines[2], 6); // data row

        // The pipe in the name must be escaped; quotes are valid markdown
        // and pass through unchanged.
        assert!(
            lines[2].contains("Gain \\| \"main\""),
            "name not escaped properly: {}",
            lines[2]
        );
        assert!(
            lines[2].contains("dB\\|FS"),
            "unit pipe not escaped: {}",
            lines[2]
        );
        assert!(
            lines[2].contains("<br>"),
            "newline in doc not converted: {}",
            lines[2]
        );
    }

    #[test]
    fn plugin_page_yaml_frontmatter_is_valid_for_quotes_and_newlines() {
        let entry = PluginEntry {
            slug: "evil",
            name: "Plugin \"X\" \\ alpha",
            description: "Adds \"warmth\".\nLine two with a \\ backslash.",
            params: &[],
            global_params: None,
            band_template: None,
        };

        let page = generate_plugin_page(&entry);
        let mut lines = page.lines();

        assert_eq!(lines.next(), Some("---"), "missing opening frontmatter");
        let title_line = lines.next().expect("title line");
        let desc_line = lines.next().expect("description line");
        let close = lines.next().expect("closing frontmatter line");
        assert_eq!(close, "---", "frontmatter must close after title+description");

        assert!(
            title_line.starts_with("title: \"") && title_line.ends_with('"'),
            "title line not properly quoted: {title_line:?}"
        );
        assert!(
            desc_line.starts_with("description: \"") && desc_line.ends_with('"'),
            "description line not properly quoted: {desc_line:?}"
        );

        // No raw newlines or carriage returns inside the quoted values.
        let title_inner = &title_line["title: \"".len()..title_line.len() - 1];
        let desc_inner = &desc_line["description: \"".len()..desc_line.len() - 1];
        assert!(!title_inner.contains('\n') && !title_inner.contains('\r'));
        assert!(!desc_inner.contains('\n') && !desc_inner.contains('\r'));

        // Quotes must be escaped, backslashes doubled.
        assert!(
            title_line.contains("\\\"X\\\""),
            "title quote not escaped: {title_line:?}"
        );
        assert!(
            title_line.contains("\\\\"),
            "title backslash not escaped: {title_line:?}"
        );
        assert!(
            desc_line.contains("\\\"warmth\\\""),
            "desc quote not escaped: {desc_line:?}"
        );
        assert!(
            desc_line.contains("\\\\"),
            "desc backslash not escaped: {desc_line:?}"
        );
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn choice_default_index_out_of_range_panics() {
        let spec = ParamSpec {
            name: "Mode",
            engine_key: "mode",
            param_type: ParamType::Choice {
                default_index: 99,
                labels: &["A", "B"],
            },
            unit: "",
            group: "",
            update_mode: UpdateMode::Realtime,
            display_scale: 1.0,
            category: ParamCategory::Primary,
            doc: "",
        };
        let _ = format_default(&spec);
    }

    #[test]
    fn registry_has_no_duplicate_slugs() {
        use std::collections::HashSet;
        let reg = plugin_registry();
        let mut seen: HashSet<&str> = HashSet::new();
        for e in &reg {
            assert!(
                seen.insert(e.slug),
                "duplicate slug in plugin registry: {}",
                e.slug
            );
        }
    }
}
