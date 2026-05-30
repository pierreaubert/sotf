//! Export DSP chain output to external audio processing formats
//!
//! Supports:
//! - CamillaDSP (YAML config)
//! - Equalizer APO / Peace GUI (text config)
//! - EasyEffects (JSON preset)
//! - Wavelet (GraphicEQ text)
//! - PipeWire filter-chain (SPA-JSON .conf)

use super::types::{ChannelDspChain, DspChainOutput, PluginConfigWrapper};
use anyhow::Context;
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::path::{Path, PathBuf};

/// Supported export formats for DSP chain output
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ExportFormat {
    /// CamillaDSP YAML configuration
    #[value(name = "camilladsp")]
    CamillaDsp,
    /// Equalizer APO / Peace GUI text format (also works with PipeWire parametric-equalizer module)
    #[value(name = "apo")]
    EqualizerApo,
    /// EasyEffects JSON preset
    #[value(name = "easyeffects")]
    EasyEffects,
    /// Wavelet GraphicEQ text format
    #[value(name = "wavelet")]
    Wavelet,
    /// PipeWire filter-chain SPA-JSON configuration
    #[value(name = "pipewire")]
    PipeWire,
    /// Roon DSP Engine preset (JSON)
    #[value(name = "roon")]
    RoonDsp,
}

impl ExportFormat {
    pub fn default_extension(&self) -> &'static str {
        match self {
            ExportFormat::CamillaDsp => "yaml",
            ExportFormat::EqualizerApo => "txt",
            ExportFormat::EasyEffects => "json",
            ExportFormat::Wavelet => "txt",
            ExportFormat::PipeWire => "conf",
            ExportFormat::RoonDsp => "json",
        }
    }

    pub fn default_file_name(&self) -> &'static str {
        match self {
            ExportFormat::CamillaDsp => "room_eq_cdsp.yaml",
            ExportFormat::EqualizerApo => "room_eq.txt",
            ExportFormat::EasyEffects => "room_eq.json",
            ExportFormat::Wavelet => "room_eq.txt",
            ExportFormat::PipeWire => "room_eq.conf",
            ExportFormat::RoonDsp => "room_eq.json",
        }
    }

    pub fn default_export_path(&self, output_path: &Path) -> PathBuf {
        if matches!(self, ExportFormat::CamillaDsp)
            && let Some(stem) = output_path.file_stem().and_then(|stem| stem.to_str())
        {
            let mut path = output_path.to_path_buf();
            path.set_file_name(format!("{stem}_cdsp.{}", self.default_extension()));
            return path;
        }

        output_path.with_extension(self.default_extension())
    }
}

/// Export a DSP chain output to the specified format
pub fn export_dsp_chain(
    output: &DspChainOutput,
    format: ExportFormat,
    path: &Path,
    sample_rate: f64,
) -> anyhow::Result<()> {
    ensure_external_export_supported(output, format)?;
    let content = match format {
        ExportFormat::CamillaDsp => export_camilladsp(output, sample_rate)?,
        ExportFormat::EqualizerApo => export_equalizer_apo(output)?,
        ExportFormat::EasyEffects => export_easyeffects(output)?,
        ExportFormat::Wavelet => export_wavelet(output, sample_rate)?,
        ExportFormat::PipeWire => export_pipewire(output, sample_rate)?,
        ExportFormat::RoonDsp => export_roon(output)?,
    };
    std::fs::write(path, content)?;
    Ok(())
}

/// Export a DSP chain and package convolution WAV sidecars beside the export
/// when the target format keeps `ir_file` references.
pub fn export_dsp_chain_with_convolution_sidecars(
    output: &DspChainOutput,
    format: ExportFormat,
    path: &Path,
    sample_rate: f64,
    source_dir: &Path,
) -> anyhow::Result<()> {
    let export_output = if export_format_preserves_convolution_paths(format) {
        let dest_dir = path.parent().unwrap_or_else(|| Path::new("."));
        package_convolution_sidecars(output, source_dir, dest_dir)?
    } else {
        output.clone()
    };
    export_dsp_chain(&export_output, format, path, sample_rate)
}

/// Copy convolution WAV sidecars into `dest_dir` and rewrite `ir_file`
/// parameters to relative filenames.
pub fn package_convolution_sidecars(
    output: &DspChainOutput,
    source_dir: &Path,
    dest_dir: &Path,
) -> anyhow::Result<DspChainOutput> {
    std::fs::create_dir_all(dest_dir)
        .with_context(|| format!("failed to create export directory '{}'", dest_dir.display()))?;

    let mut output = output.clone();
    let mut copied = HashMap::new();
    rewrite_convolution_sidecars(
        &mut output.global_plugins,
        source_dir,
        dest_dir,
        &mut copied,
    )?;
    for chain in output.channels.values_mut() {
        rewrite_convolution_sidecars(&mut chain.plugins, source_dir, dest_dir, &mut copied)?;
        if let Some(drivers) = chain.drivers.as_mut() {
            for driver in drivers {
                rewrite_convolution_sidecars(
                    &mut driver.plugins,
                    source_dir,
                    dest_dir,
                    &mut copied,
                )?;
            }
        }
    }

    Ok(output)
}

pub fn external_export_supported(
    output: &DspChainOutput,
    format: ExportFormat,
) -> anyhow::Result<()> {
    ensure_external_export_supported(output, format)
}

fn ensure_external_export_supported(
    output: &DspChainOutput,
    format: ExportFormat,
) -> anyhow::Result<()> {
    if output.global_plugins.is_empty()
        && output
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.bass_management.as_ref())
            .and_then(|report| report.routing_graph.as_ref())
            .is_none_or(|graph| graph.routes.is_empty())
    {
        return Ok(());
    }

    anyhow::bail!(
        "{format:?} export cannot represent routed home-cinema bass management safely. \
         Use SotF JSON or Apply as Graph so global_plugins and route-level bass-management DSP are preserved."
    );
}

fn export_format_preserves_convolution_paths(format: ExportFormat) -> bool {
    matches!(
        format,
        ExportFormat::CamillaDsp | ExportFormat::EqualizerApo | ExportFormat::RoonDsp
    )
}

fn rewrite_convolution_sidecars(
    plugins: &mut [PluginConfigWrapper],
    source_dir: &Path,
    dest_dir: &Path,
    copied: &mut HashMap<PathBuf, String>,
) -> anyhow::Result<()> {
    for plugin in plugins {
        if plugin.plugin_type != "convolution" {
            continue;
        }

        let Some(ir_file) = plugin
            .parameters
            .get("ir_file")
            .and_then(|value| value.as_str())
        else {
            continue;
        };
        let packaged_name = package_one_convolution_sidecar(ir_file, source_dir, dest_dir, copied)?;
        let Some(params) = plugin.parameters.as_object_mut() else {
            anyhow::bail!("convolution plugin parameters must be a JSON object");
        };
        params.insert("ir_file".to_string(), serde_json::json!(packaged_name));
    }

    Ok(())
}

fn package_one_convolution_sidecar(
    ir_file: &str,
    source_dir: &Path,
    dest_dir: &Path,
    copied: &mut HashMap<PathBuf, String>,
) -> anyhow::Result<String> {
    let ir_path = Path::new(ir_file);
    let source_path = if ir_path.is_absolute() {
        ir_path.to_path_buf()
    } else {
        source_dir.join(ir_path)
    };
    let source_path = source_path.canonicalize().with_context(|| {
        format!(
            "convolution WAV '{}' was not found relative to '{}'",
            ir_file,
            source_dir.display()
        )
    })?;

    if let Some(existing) = copied.get(&source_path) {
        return Ok(existing.clone());
    }

    let preferred = ir_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("room_eq_ir.wav");
    let filename = unique_sidecar_filename(dest_dir, preferred, &source_path)?;
    let dest_path = dest_dir.join(&filename);
    if !same_existing_file(&dest_path, &source_path)? {
        std::fs::copy(&source_path, &dest_path).with_context(|| {
            format!(
                "failed to copy convolution WAV '{}' to '{}'",
                source_path.display(),
                dest_path.display()
            )
        })?;
    }
    copied.insert(source_path, filename.clone());
    Ok(filename)
}

fn unique_sidecar_filename(
    dest_dir: &Path,
    preferred: &str,
    source_path: &Path,
) -> anyhow::Result<String> {
    let preferred_path = dest_dir.join(preferred);
    if !preferred_path.exists() || same_existing_file(&preferred_path, source_path)? {
        return Ok(preferred.to_string());
    }

    let preferred_path = Path::new(preferred);
    let stem = preferred_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("room_eq_ir");
    let ext = preferred_path
        .extension()
        .and_then(|ext| ext.to_str())
        .filter(|ext| !ext.is_empty())
        .map(|ext| format!(".{ext}"))
        .unwrap_or_default();

    for suffix in 2.. {
        let candidate = format!("{stem}_{suffix:03}{ext}");
        let candidate_path = dest_dir.join(&candidate);
        if !candidate_path.exists() || same_existing_file(&candidate_path, source_path)? {
            return Ok(candidate);
        }
    }

    unreachable!("unbounded numeric suffix search must return a filename")
}

fn same_existing_file(path: &Path, source_path: &Path) -> anyhow::Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    Ok(path.canonicalize()? == source_path)
}

// ============================================================================
// Internal data extraction helpers
// ============================================================================

struct BiquadExport {
    filter_type: String,
    freq: f64,
    q: f64,
    gain_db: f64,
}

/// Extract all biquad filters from a plugin list
fn extract_eq_filters(plugins: &[PluginConfigWrapper]) -> Vec<BiquadExport> {
    let mut filters = Vec::new();
    for p in plugins {
        if p.plugin_type == "eq"
            && let Some(arr) = p.parameters.get("filters").and_then(|v| v.as_array())
        {
            for f in arr {
                filters.push(BiquadExport {
                    filter_type: f
                        .get("filter_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("peak")
                        .to_string(),
                    freq: f.get("freq").and_then(|v| v.as_f64()).unwrap_or(1000.0),
                    q: f.get("q").and_then(|v| v.as_f64()).unwrap_or(1.0),
                    gain_db: f.get("db_gain").and_then(|v| v.as_f64()).unwrap_or(0.0),
                });
            }
        }
    }
    filters
}

/// Sum all gain values from gain plugins
fn extract_gain_db(plugins: &[PluginConfigWrapper]) -> f64 {
    plugins
        .iter()
        .filter(|p| p.plugin_type == "gain")
        .filter_map(|p| p.parameters.get("gain_db").and_then(|v| v.as_f64()))
        .sum()
}

/// Extract delay in ms (sum of all delay plugins)
fn extract_delay_ms(plugins: &[PluginConfigWrapper]) -> Option<f64> {
    let total: f64 = plugins
        .iter()
        .filter(|p| p.plugin_type == "delay")
        .filter_map(|p| p.parameters.get("delay_ms").and_then(|v| v.as_f64()))
        .sum();
    if total.abs() > 0.001 {
        Some(total)
    } else {
        None
    }
}

/// Extract convolution IR file paths
fn extract_convolution_paths(plugins: &[PluginConfigWrapper]) -> Vec<String> {
    plugins
        .iter()
        .filter(|p| p.plugin_type == "convolution")
        .filter_map(|p| {
            p.parameters
                .get("ir_file")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect()
}

/// Collect all plugins from a channel: combined (channel-level) + per-driver plugins
fn collect_all_plugins(chain: &ChannelDspChain) -> Vec<&PluginConfigWrapper> {
    let mut all = Vec::new();
    if let Some(drivers) = &chain.drivers {
        for driver in drivers {
            all.extend(driver.plugins.iter());
        }
    }
    all.extend(chain.plugins.iter());
    all
}

/// Map channel name to standard short name
fn channel_short_name(name: &str) -> &str {
    match name {
        "left" => "L",
        "right" => "R",
        "center" => "C",
        "lfe" | "sub" | "subwoofer" => "LFE",
        "surround_left" => "SL",
        "surround_right" => "SR",
        "back_left" => "BL",
        "back_right" => "BR",
        other => other,
    }
}

/// Map channel name to standard channel index (None for unknown channels)
fn channel_index(name: &str) -> Option<usize> {
    match name {
        "left" => Some(0),
        "right" => Some(1),
        "center" => Some(2),
        "lfe" | "sub" | "subwoofer" => Some(3),
        "surround_left" => Some(4),
        "surround_right" => Some(5),
        "back_left" => Some(6),
        "back_right" => Some(7),
        _ => None,
    }
}

/// Get sorted channel list for deterministic output.
/// Known channels sort by standard order; unknown channels sort alphabetically after.
fn sorted_channels(output: &DspChainOutput) -> Vec<(&String, &ChannelDspChain)> {
    let mut channels: Vec<_> = output.channels.iter().collect();
    channels.sort_by(|(a, _), (b, _)| {
        let ia = channel_index(a);
        let ib = channel_index(b);
        match (ia, ib) {
            // Both known: sort by index
            (Some(a_idx), Some(b_idx)) => a_idx.cmp(&b_idx),
            // Known before unknown
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            // Both unknown: alphabetical
            (None, None) => a.cmp(b),
        }
    });
    channels
}

/// Map filter type string to CamillaDSP filter type
fn camilladsp_filter_type(ft: &str) -> &str {
    match ft {
        "peak" => "Peaking",
        "lowshelf" => "Lowshelf",
        "highshelf" => "Highshelf",
        "lowpass" => "Lowpass",
        "highpass" | "highpassvariableq" => "Highpass",
        "notch" => "Notch",
        "bandpass" => "Bandpass",
        "allpass" => "Allpass",
        other => other,
    }
}

/// Map filter type string to APO filter abbreviation
fn apo_filter_type(ft: &str) -> &str {
    match ft {
        "peak" => "PK",
        "lowshelf" => "LSC",
        "highshelf" => "HSC",
        "lowpass" => "LP",
        "highpass" | "highpassvariableq" => "HP",
        "notch" => "NO",
        "bandpass" => "BP",
        "allpass" => "AP",
        other => other,
    }
}

/// Map filter type string to EasyEffects type
fn easyeffects_filter_type(ft: &str) -> &str {
    match ft {
        "peak" => "Bell",
        "lowshelf" => "Lo Shelf",
        "highshelf" => "Hi Shelf",
        "lowpass" => "Lo-pass",
        "highpass" | "highpassvariableq" => "Hi-pass",
        "notch" => "Notch",
        "bandpass" => "Bandpass",
        "allpass" => "Allpass",
        other => other,
    }
}

/// Map filter type string to PipeWire builtin label
fn pipewire_filter_label(ft: &str) -> anyhow::Result<&'static str> {
    let label = match ft {
        "peak" => "bq_peaking",
        "lowshelf" => "bq_lowshelf",
        "highshelf" => "bq_highshelf",
        "lowpass" => "bq_lowpass",
        "highpass" | "highpassvariableq" => "bq_highpass",
        "notch" => "bq_notch",
        "bandpass" => "bq_bandpass",
        "allpass" => "bq_allpass",
        other => anyhow::bail!("Unsupported PipeWire biquad filter type '{other}'"),
    };

    Ok(label)
}

// ============================================================================
// CamillaDSP export
// ============================================================================

fn export_camilladsp(output: &DspChainOutput, sample_rate: f64) -> anyhow::Result<String> {
    let mut out = String::new();
    writeln!(out, "# CamillaDSP configuration")?;
    writeln!(out, "# Generated by roomeq")?;
    writeln!(out)?;

    let channels = sorted_channels(output);
    let num_channels = channels.len();

    // Devices section
    writeln!(out, "devices:")?;
    writeln!(out, "  samplerate: {}", sample_rate as u32)?;
    writeln!(out, "  chunksize: 4096")?;
    writeln!(out, "  capture:")?;
    writeln!(out, "    type: File")?;
    writeln!(out, "    channels: {num_channels}")?;
    writeln!(out, "    filename: /dev/stdin")?;
    writeln!(out, "    format: S32LE")?;
    writeln!(out, "  playback:")?;
    writeln!(out, "    type: File")?;
    writeln!(out, "    channels: {num_channels}")?;
    writeln!(out, "    filename: /dev/stdout")?;
    writeln!(out, "    format: S32LE")?;
    writeln!(out)?;

    // Filters section
    writeln!(out, "filters:")?;

    for (ch_name, chain) in &channels {
        let prefix = ch_name.replace(' ', "_");
        write_camilladsp_filters_for_plugins(&mut out, &prefix, &chain.plugins, "")?;

        if let Some(drivers) = &chain.drivers {
            for driver in drivers {
                let driver_prefix = format!("{}_{}", prefix, driver.name.replace(' ', "_"));
                write_camilladsp_filters_for_plugins(
                    &mut out,
                    &driver_prefix,
                    &driver.plugins,
                    "",
                )?;
            }
        }
    }
    writeln!(out)?;

    // Pipeline section
    writeln!(out, "pipeline:")?;

    for (i, (ch_name, chain)) in channels.iter().enumerate() {
        let prefix = ch_name.replace(' ', "_");

        // Driver-level filters first
        if let Some(drivers) = &chain.drivers {
            writeln!(out, "# Channel: {} (drivers)", ch_name)?;
            for driver in drivers {
                let driver_prefix = format!("{}_{}", prefix, driver.name.replace(' ', "_"));
                let filter_names =
                    collect_camilladsp_filter_names(&driver_prefix, &driver.plugins, "");
                if !filter_names.is_empty() {
                    write_camilladsp_pipeline_filter_step(&mut out, i, &filter_names)?;
                }
            }
        }

        // Channel-level filters
        let filter_names = collect_camilladsp_filter_names(&prefix, &chain.plugins, "");
        if !filter_names.is_empty() {
            write_camilladsp_pipeline_filter_step(&mut out, i, &filter_names)?;
        }
    }

    Ok(out)
}

fn write_camilladsp_pipeline_filter_step(
    out: &mut String,
    channel_index: usize,
    filter_names: &[String],
) -> anyhow::Result<()> {
    writeln!(out, "- bypassed: null")?;
    writeln!(out, "  channels:")?;
    writeln!(out, "  - {channel_index}")?;
    writeln!(out, "  names:")?;
    for name in filter_names {
        writeln!(out, "  - {name}")?;
    }
    writeln!(out, "  type: Filter")?;
    Ok(())
}

fn write_camilladsp_filters_for_plugins(
    out: &mut String,
    prefix: &str,
    plugins: &[PluginConfigWrapper],
    _suffix: &str,
) -> anyhow::Result<()> {
    let mut eq_idx = 0;
    let mut gain_idx = 0;
    let mut delay_idx = 0;
    let mut conv_idx = 0;

    for plugin in plugins {
        match plugin.plugin_type.as_str() {
            "gain" => {
                let gain_db = plugin
                    .parameters
                    .get("gain_db")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let inverted = plugin
                    .parameters
                    .get("invert")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let name = if gain_idx == 0 {
                    format!("{prefix}_gain")
                } else {
                    format!("{prefix}_gain_{gain_idx}")
                };
                writeln!(out, "  {name}:")?;
                writeln!(out, "    type: Gain")?;
                writeln!(out, "    parameters:")?;
                writeln!(out, "      gain: {gain_db:.2}")?;
                if inverted {
                    writeln!(out, "      inverted: true")?;
                }
                gain_idx += 1;
            }
            "delay" => {
                let delay_ms = plugin
                    .parameters
                    .get("delay_ms")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let name = if delay_idx == 0 {
                    format!("{prefix}_delay")
                } else {
                    format!("{prefix}_delay_{delay_idx}")
                };
                writeln!(out, "  {name}:")?;
                writeln!(out, "    type: Delay")?;
                writeln!(out, "    parameters:")?;
                writeln!(out, "      delay: {delay_ms:.3}")?;
                writeln!(out, "      unit: ms")?;
                delay_idx += 1;
            }
            "eq" => {
                if let Some(filters) = plugin.parameters.get("filters").and_then(|v| v.as_array()) {
                    for f in filters {
                        let ft = f
                            .get("filter_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("peak");
                        let freq = f.get("freq").and_then(|v| v.as_f64()).unwrap_or(1000.0);
                        let q = f.get("q").and_then(|v| v.as_f64()).unwrap_or(1.0);
                        let gain = f.get("db_gain").and_then(|v| v.as_f64()).unwrap_or(0.0);

                        writeln!(out, "  {prefix}_peq_{eq_idx}:")?;
                        writeln!(out, "    type: Biquad")?;
                        writeln!(out, "    parameters:")?;
                        writeln!(out, "      type: {}", camilladsp_filter_type(ft))?;
                        writeln!(out, "      freq: {freq:.1}")?;
                        writeln!(out, "      q: {q:.4}")?;
                        match ft {
                            "lowpass" | "highpass" | "notch" | "bandpass" | "allpass" => {}
                            _ => {
                                writeln!(out, "      gain: {gain:.2}")?;
                            }
                        }
                        eq_idx += 1;
                    }
                }
            }
            "convolution" => {
                if let Some(ir_file) = plugin.parameters.get("ir_file").and_then(|v| v.as_str()) {
                    let name = if conv_idx == 0 {
                        format!("{prefix}_conv")
                    } else {
                        format!("{prefix}_conv_{conv_idx}")
                    };
                    writeln!(out, "  {name}:")?;
                    writeln!(out, "    type: Conv")?;
                    writeln!(out, "    parameters:")?;
                    writeln!(out, "      type: Wav")?;
                    writeln!(out, "      filename: {ir_file}")?;
                    conv_idx += 1;
                }
            }
            _ => {} // Skip unsupported plugin types
        }
    }
    Ok(())
}

fn collect_camilladsp_filter_names(
    prefix: &str,
    plugins: &[PluginConfigWrapper],
    _suffix: &str,
) -> Vec<String> {
    let mut names = Vec::new();
    let mut eq_idx = 0;
    let mut gain_idx = 0;
    let mut delay_idx = 0;
    let mut conv_idx = 0;

    for plugin in plugins {
        match plugin.plugin_type.as_str() {
            "gain" => {
                names.push(if gain_idx == 0 {
                    format!("{prefix}_gain")
                } else {
                    format!("{prefix}_gain_{gain_idx}")
                });
                gain_idx += 1;
            }
            "delay" => {
                names.push(if delay_idx == 0 {
                    format!("{prefix}_delay")
                } else {
                    format!("{prefix}_delay_{delay_idx}")
                });
                delay_idx += 1;
            }
            "eq" => {
                if let Some(filters) = plugin.parameters.get("filters").and_then(|v| v.as_array()) {
                    for _ in filters {
                        names.push(format!("{prefix}_peq_{eq_idx}"));
                        eq_idx += 1;
                    }
                }
            }
            "convolution" => {
                names.push(if conv_idx == 0 {
                    format!("{prefix}_conv")
                } else {
                    format!("{prefix}_conv_{conv_idx}")
                });
                conv_idx += 1;
            }
            _ => {}
        }
    }
    names
}

// ============================================================================
// Equalizer APO export
// ============================================================================

fn export_equalizer_apo(output: &DspChainOutput) -> anyhow::Result<String> {
    let mut out = String::new();
    writeln!(out, "# Equalizer APO configuration")?;
    writeln!(out, "# Generated by roomeq")?;
    writeln!(out)?;

    let channels = sorted_channels(output);

    for (ch_name, chain) in &channels {
        let short = channel_short_name(ch_name);
        writeln!(out, "Channel: {short}")?;

        // Collect all plugins from channel + drivers
        let plugins: Vec<PluginConfigWrapper> =
            collect_all_plugins(chain).into_iter().cloned().collect();

        // Gain (preamp)
        let gain = extract_gain_db(&plugins);
        if gain.abs() > 0.01 {
            writeln!(out, "Preamp: {gain:+.1} dB")?;
        }

        // Delay
        if let Some(delay) = extract_delay_ms(&plugins) {
            writeln!(out, "Delay: {delay:.3} ms")?;
        }

        // EQ filters
        let filters = extract_eq_filters(&plugins);
        for (i, f) in filters.iter().enumerate() {
            let ft = apo_filter_type(&f.filter_type);
            match f.filter_type.as_str() {
                "lowpass" | "highpass" | "highpassvariableq" => {
                    writeln!(out, "Filter {:2}: ON {ft} Fc {:.0} Hz", i + 1, f.freq)?;
                }
                _ => {
                    writeln!(
                        out,
                        "Filter {:2}: ON {ft} Fc {:.0} Hz Gain {:+.2} dB Q {:.4}",
                        i + 1,
                        f.freq,
                        f.gain_db,
                        f.q
                    )?;
                }
            }
        }

        // Convolution
        let conv_paths = extract_convolution_paths(&plugins);
        for path in &conv_paths {
            writeln!(out, "Convolution: {path}")?;
        }

        writeln!(out)?;
    }

    Ok(out)
}

// ============================================================================
// EasyEffects export
// ============================================================================

fn export_easyeffects(output: &DspChainOutput) -> anyhow::Result<String> {
    let channels = sorted_channels(output);

    // Merge all channel EQ filters into one preset (EasyEffects applies globally)
    let mut all_filters = Vec::new();
    let mut min_gain = 0.0f64;

    // Check for convolution/FIR and warn
    let mut has_unsupported = false;

    for (ch_name, chain) in &channels {
        let plugins: Vec<PluginConfigWrapper> =
            collect_all_plugins(chain).into_iter().cloned().collect();
        let filters = extract_eq_filters(&plugins);
        let gain = extract_gain_db(&plugins);

        if !extract_convolution_paths(&plugins).is_empty() {
            has_unsupported = true;
            log::warn!(
                "EasyEffects: skipping convolution for channel '{}' (not supported)",
                ch_name
            );
        }

        // Use most negative gain as preamp to prevent clipping
        if gain < min_gain {
            min_gain = gain;
        }

        all_filters.extend(filters);
    }

    if has_unsupported {
        log::warn!("EasyEffects does not support FIR convolution filters; they were skipped");
    }

    // Build JSON preset
    let mut bands = serde_json::Map::new();
    for (i, f) in all_filters.iter().enumerate().take(30) {
        let band_key = format!("band{i}");
        let mut band = serde_json::Map::new();
        band.insert("frequency".to_string(), serde_json::json!(f.freq));
        band.insert("gain".to_string(), serde_json::json!(f.gain_db));
        band.insert("q".to_string(), serde_json::json!(f.q));
        band.insert(
            "type".to_string(),
            serde_json::json!(easyeffects_filter_type(&f.filter_type)),
        );
        band.insert("mode".to_string(), serde_json::json!("RLC (BT)"));
        band.insert("slope".to_string(), serde_json::json!("x1"));
        band.insert("solo".to_string(), serde_json::json!(false));
        band.insert("mute".to_string(), serde_json::json!(false));
        bands.insert(band_key, serde_json::Value::Object(band));
    }

    let preset = serde_json::json!({
        "output": {
            "equalizer#0": {
                "input-gain": min_gain,
                "output-gain": 0.0,
                "num-bands": all_filters.len().min(30),
                "split-channels": false,
                "left": bands,
                "right": bands,
            }
        }
    });

    Ok(serde_json::to_string_pretty(&preset)?)
}

// ============================================================================
// Wavelet export
// ============================================================================

/// Fixed graphic EQ band center frequencies for Wavelet
const WAVELET_BANDS: [f64; 9] = [
    32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0,
];

fn export_wavelet(output: &DspChainOutput, sample_rate: f64) -> anyhow::Result<String> {
    let channels = sorted_channels(output);

    let mut has_unsupported = false;

    // Average the response across all channels
    let mut band_gains = [0.0f64; 9];
    let mut n_channels = 0;

    for (ch_name, chain) in &channels {
        let plugins: Vec<PluginConfigWrapper> =
            collect_all_plugins(chain).into_iter().cloned().collect();
        let filters = extract_eq_filters(&plugins);
        let gain = extract_gain_db(&plugins);

        if !extract_convolution_paths(&plugins).is_empty() {
            has_unsupported = true;
            log::warn!(
                "Wavelet: skipping convolution for channel '{}' (not supported)",
                ch_name
            );
        }

        // Build Biquad chain and evaluate at each band frequency
        let biquads: Vec<Biquad> = filters
            .iter()
            .map(|f| {
                let ft = parse_biquad_filter_type(&f.filter_type)?;
                Ok(Biquad::new(ft, f.freq, sample_rate, f.q, f.gain_db))
            })
            .collect::<anyhow::Result<_>>()?;

        for (i, &freq) in WAVELET_BANDS.iter().enumerate() {
            let mut db = gain;
            for bq in &biquads {
                db += bq.log_result(freq);
            }
            band_gains[i] += db;
        }
        n_channels += 1;
    }

    if has_unsupported {
        log::warn!("Wavelet does not support FIR convolution filters; they were skipped");
    }

    // Average across channels
    if n_channels > 1 {
        for g in &mut band_gains {
            *g /= n_channels as f64;
        }
    }

    // Format output
    let mut out = String::new();
    writeln!(out, "# Wavelet GraphicEQ")?;
    writeln!(out, "# Generated by roomeq")?;
    write!(out, "GraphicEQ:")?;
    for (i, (&freq, &gain)) in WAVELET_BANDS.iter().zip(band_gains.iter()).enumerate() {
        if i > 0 {
            write!(out, ";")?;
        }
        write!(out, " {:.0} {:.1}", freq, gain)?;
    }
    writeln!(out)?;

    Ok(out)
}

/// Parse filter type string to BiquadFilterType enum
fn parse_biquad_filter_type(ft: &str) -> anyhow::Result<BiquadFilterType> {
    let filter_type = match ft {
        "peak" => BiquadFilterType::Peak,
        "lowshelf" => BiquadFilterType::Lowshelf,
        "highshelf" => BiquadFilterType::Highshelf,
        "lowpass" => BiquadFilterType::Lowpass,
        "highpass" => BiquadFilterType::Highpass,
        "highpassvariableq" => BiquadFilterType::HighpassVariableQ,
        "notch" => BiquadFilterType::Notch,
        "bandpass" => BiquadFilterType::Bandpass,
        "allpass" => BiquadFilterType::AllPass,
        other => anyhow::bail!("Unsupported biquad filter type '{other}'"),
    };

    Ok(filter_type)
}

// ============================================================================
// PipeWire filter-chain export
// ============================================================================

fn export_pipewire(output: &DspChainOutput, sample_rate: f64) -> anyhow::Result<String> {
    let mut out = String::new();
    writeln!(out, "# PipeWire filter-chain configuration")?;
    writeln!(out, "# Generated by roomeq")?;
    writeln!(out)?;
    writeln!(out, "context.modules = [")?;
    writeln!(out, "  {{ name = libpipewire-module-filter-chain")?;
    writeln!(out, "    args = {{")?;

    let channels = sorted_channels(output);
    let num_channels = channels.len();

    // Build channel position list
    let positions: Vec<&str> = channels
        .iter()
        .map(|(name, _)| channel_short_name(name))
        .collect();
    let positions_str = positions
        .iter()
        .map(|p| format!("\"{}\"", pipewire_channel_position(p)))
        .collect::<Vec<_>>()
        .join(", ");

    // Nodes
    writeln!(out, "      filter.graph = {{")?;
    writeln!(out, "        nodes = [")?;

    let mut all_node_names: Vec<Vec<String>> = Vec::new(); // per-channel node names

    for (ch_idx, (ch_name, chain)) in channels.iter().enumerate() {
        let ch_prefix = format!("ch{}_{}", ch_idx, ch_name.replace(' ', "_"));
        let mut node_names = Vec::new();

        // Collect all plugins
        let all_plugins: Vec<PluginConfigWrapper> =
            collect_all_plugins(chain).into_iter().cloned().collect();

        // Gain node
        let gain = extract_gain_db(&all_plugins);
        if gain.abs() > 0.01 {
            let node_name = format!("{ch_prefix}_gain");
            writeln!(
                out,
                "          {{ type = builtin  name = \"{node_name}\"  label = bq_highshelf  control = {{ \"Freq\" = 0  \"Q\" = 1.0  \"Gain\" = {gain:.2} }} }}"
            )?;
            node_names.push(node_name);
        }

        // Delay node (PipeWire uses delay builtin)
        if let Some(delay_ms) = extract_delay_ms(&all_plugins) {
            let delay_samples = (delay_ms / 1000.0 * sample_rate).round() as u64;
            let node_name = format!("{ch_prefix}_delay");
            writeln!(
                out,
                "          {{ type = builtin  name = \"{node_name}\"  label = delay  control = {{ \"Delay\" = {delay_samples} }} }}"
            )?;
            node_names.push(node_name);
        }

        // EQ filter nodes
        let filters = extract_eq_filters(&all_plugins);
        for (i, f) in filters.iter().enumerate() {
            let label = pipewire_filter_label(&f.filter_type)?;
            let node_name = format!("{ch_prefix}_eq_{i}");
            match f.filter_type.as_str() {
                "lowpass" | "highpass" | "highpassvariableq" => {
                    writeln!(
                        out,
                        "          {{ type = builtin  name = \"{node_name}\"  label = {label}  control = {{ \"Freq\" = {:.1}  \"Q\" = {:.4} }} }}",
                        f.freq, f.q
                    )?;
                }
                _ => {
                    writeln!(
                        out,
                        "          {{ type = builtin  name = \"{node_name}\"  label = {label}  control = {{ \"Freq\" = {:.1}  \"Q\" = {:.4}  \"Gain\" = {:.2} }} }}",
                        f.freq, f.q, f.gain_db
                    )?;
                }
            }
            node_names.push(node_name);
        }

        all_node_names.push(node_names);
    }

    writeln!(out, "        ]")?;

    // Links: chain nodes sequentially per channel
    writeln!(out, "        links = [")?;
    for nodes in &all_node_names {
        for pair in nodes.windows(2) {
            writeln!(
                out,
                "          {{ output = \"{}:Out\"  input = \"{}:In\" }}",
                pair[0], pair[1]
            )?;
        }
    }
    writeln!(out, "        ]")?;

    // Inputs/outputs
    writeln!(out, "        inputs  = [")?;
    for (ch_idx, nodes) in all_node_names.iter().enumerate() {
        if let Some(first) = nodes.first() {
            writeln!(out, "          {{ node = \"{first}\"  port = \"In\" }}")?;
        } else {
            // No processing nodes — passthrough
            writeln!(out, "          {{ }}")?;
        }
        let _ = ch_idx; // suppress warning
    }
    writeln!(out, "        ]")?;

    writeln!(out, "        outputs = [")?;
    for nodes in &all_node_names {
        if let Some(last) = nodes.last() {
            writeln!(out, "          {{ node = \"{last}\"  port = \"Out\" }}")?;
        } else {
            writeln!(out, "          {{ }}")?;
        }
    }
    writeln!(out, "        ]")?;

    writeln!(out, "      }}")?; // filter.graph

    // Capture/playback props
    writeln!(out, "      capture.props = {{")?;
    writeln!(out, "        audio.channels = {num_channels}")?;
    writeln!(out, "        audio.position = [ {positions_str} ]")?;
    writeln!(out, "      }}")?;
    writeln!(out, "      playback.props = {{")?;
    writeln!(out, "        audio.channels = {num_channels}")?;
    writeln!(out, "        audio.position = [ {positions_str} ]")?;
    writeln!(out, "      }}")?;

    writeln!(out, "    }}")?; // args
    writeln!(out, "  }}")?; // module
    writeln!(out, "]")?;

    Ok(out)
}

/// Map short channel name to PipeWire position string
fn pipewire_channel_position(short: &str) -> &str {
    match short {
        "L" => "FL",
        "R" => "FR",
        "C" => "FC",
        "LFE" => "LFE",
        "SL" => "SL",
        "SR" => "SR",
        "BL" => "RL",
        "BR" => "RR",
        other => other,
    }
}

// ============================================================================
// Roon DSP export
// ============================================================================

/// Map filter type string to Roon parametric EQ type
fn roon_filter_type(ft: &str) -> &str {
    match ft {
        "peak" => "Peak/Dip",
        "lowshelf" => "Low Shelf",
        "highshelf" => "High Shelf",
        "lowpass" => "Low Pass",
        "highpass" | "highpassvariableq" => "High Pass",
        "bandpass" => "Band Pass",
        "notch" => "Band Stop",
        "allpass" => "Band Stop", // Roon has no allpass; closest equivalent
        other => other,
    }
}

/// Export a DSP chain as a Roon DSP Engine preset.
///
/// Roon's DSP Engine supports per-channel parametric EQ (up to 20 bands),
/// headroom management (preamp gain), and convolution (WAV files).
/// The output is a JSON object with per-channel sections that can be
/// referenced when configuring Roon's DSP Engine manually.
fn export_roon(output: &DspChainOutput) -> anyhow::Result<String> {
    let channels = sorted_channels(output);

    let mut roon = serde_json::Map::new();
    roon.insert(
        "_comment".to_string(),
        serde_json::json!("Roon DSP Engine preset — generated by roomeq"),
    );

    let mut channel_configs = serde_json::Map::new();

    for (ch_name, chain) in &channels {
        let all_plugins: Vec<PluginConfigWrapper> =
            collect_all_plugins(chain).into_iter().cloned().collect();

        let mut ch = serde_json::Map::new();

        // Headroom / preamp gain
        let gain = extract_gain_db(&all_plugins);
        if gain.abs() > 0.01 {
            ch.insert("headroom_gain_db".to_string(), serde_json::json!(gain));
        }

        // Delay (Roon doesn't have a per-channel delay in PEQ, but we include it
        // so the user knows what value to set in Speaker Setup > Distance)
        if let Some(delay_ms) = extract_delay_ms(&all_plugins) {
            ch.insert("delay_ms".to_string(), serde_json::json!(delay_ms));
        }

        // Parametric EQ bands (max 20 per Roon endpoint)
        let filters = extract_eq_filters(&all_plugins);
        let mut bands = Vec::new();
        for f in filters.iter().take(20) {
            let mut band = serde_json::Map::new();
            band.insert(
                "type".to_string(),
                serde_json::json!(roon_filter_type(&f.filter_type)),
            );
            band.insert("frequency".to_string(), serde_json::json!(f.freq));
            band.insert("gain".to_string(), serde_json::json!(f.gain_db));
            band.insert("q".to_string(), serde_json::json!(f.q));
            band.insert("enabled".to_string(), serde_json::json!(true));
            bands.push(serde_json::Value::Object(band));
        }
        if !bands.is_empty() {
            ch.insert(
                "parametric_eq".to_string(),
                serde_json::json!({
                    "bands": bands,
                    "is_enabled": true
                }),
            );
        }

        // Convolution IR files
        let conv_paths = extract_convolution_paths(&all_plugins);
        if !conv_paths.is_empty() {
            ch.insert(
                "convolution".to_string(),
                serde_json::json!({
                    "ir_files": conv_paths,
                    "is_enabled": true
                }),
            );
        }

        channel_configs.insert(ch_name.to_string(), serde_json::Value::Object(ch));
    }

    roon.insert(
        "channels".to_string(),
        serde_json::Value::Object(channel_configs),
    );

    Ok(serde_json::to_string_pretty(&serde_json::Value::Object(
        roon,
    ))?)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roomeq::types::*;
    use serde_json::json;
    use std::collections::HashMap;

    /// Build a test DspChainOutput with 2 channels, each having gain + 3 PEQ + delay
    fn make_test_output() -> DspChainOutput {
        let mut channels = HashMap::new();

        // Left channel: gain -2.5 dB, delay 1.5 ms, 3 PEQ bands
        channels.insert(
            "left".to_string(),
            ChannelDspChain {
                channel: "left".to_string(),
                plugins: vec![
                    PluginConfigWrapper {
                        plugin_type: "gain".to_string(),
                        parameters: json!({"gain_db": -2.5}),
                    },
                    PluginConfigWrapper {
                        plugin_type: "delay".to_string(),
                        parameters: json!({"delay_ms": 1.5}),
                    },
                    PluginConfigWrapper {
                        plugin_type: "eq".to_string(),
                        parameters: json!({
                            "filters": [
                                {"filter_type": "peak", "freq": 100.0, "q": 2.0, "db_gain": -5.0},
                                {"filter_type": "peak", "freq": 1000.0, "q": 1.5, "db_gain": 3.0},
                                {"filter_type": "highshelf", "freq": 8000.0, "q": 0.7, "db_gain": -2.0},
                            ]
                        }),
                    },
                ],
                drivers: None,
                initial_curve: None,
                final_curve: None,
                eq_response: None,
                target_curve: None,
                pre_ir: None,
                post_ir: None,
                fir_temporal_masking: None,
        direct_early_late_correction: None,
            },
        );

        // Right channel: gain -1.0 dB, 2 PEQ bands
        channels.insert(
            "right".to_string(),
            ChannelDspChain {
                channel: "right".to_string(),
                plugins: vec![
                    PluginConfigWrapper {
                        plugin_type: "gain".to_string(),
                        parameters: json!({"gain_db": -1.0}),
                    },
                    PluginConfigWrapper {
                        plugin_type: "eq".to_string(),
                        parameters: json!({
                            "filters": [
                                {"filter_type": "peak", "freq": 200.0, "q": 1.0, "db_gain": -3.0},
                                {"filter_type": "lowshelf", "freq": 80.0, "q": 0.71, "db_gain": 4.0},
                            ]
                        }),
                    },
                ],
                drivers: None,
                initial_curve: None,
                final_curve: None,
                eq_response: None,
                target_curve: None,
                pre_ir: None,
                post_ir: None,
                fir_temporal_masking: None,
        direct_early_late_correction: None,
            },
        );

        DspChainOutput {
            version: "1.3.0".to_string(),
            global_plugins: Vec::new(),
            channels,
            metadata: Some(OptimizationMetadata {
                pre_score: 5.0,
                post_score: 2.0,
                algorithm: "cobyla".to_string(),
                loss_type: Some("flat".to_string()),
                iterations: 1000,
                timestamp: "2026-01-01T00:00:00Z".to_string(),
                inter_channel_deviation: None,
                epa_per_channel: None,
                epa_multichannel: None,
                group_delay: None,
                perceptual_metrics: None,
                home_cinema_layout: None,
                multi_seat_coverage: None,
                multi_seat_correction: None,
                bass_management: None,
                timing_diagnostics: None,
                ctc: None,
                perceptual_policy: None,
                bootstrap_uncertainty: None,
                validation_bundle: None,
            }),
        }
    }

    fn make_single_filter_output(filter_type: &str, gain_db: f64) -> DspChainOutput {
        let mut channels = HashMap::new();
        channels.insert(
            "left".to_string(),
            ChannelDspChain {
                channel: "left".to_string(),
                plugins: vec![PluginConfigWrapper {
                    plugin_type: "eq".to_string(),
                    parameters: json!({
                        "filters": [
                            {
                                "filter_type": filter_type,
                                "freq": 80.0,
                                "q": 0.707,
                                "db_gain": gain_db,
                            }
                        ]
                    }),
                }],
                drivers: None,
                initial_curve: None,
                final_curve: None,
                eq_response: None,
                target_curve: None,
                pre_ir: None,
                post_ir: None,
                fir_temporal_masking: None,
                direct_early_late_correction: None,
            },
        );

        DspChainOutput {
            version: "1.3.0".to_string(),
            global_plugins: Vec::new(),
            channels,
            metadata: None,
        }
    }

    #[test]
    fn external_exports_reject_routed_bass_management() {
        let mut output = make_test_output();
        output.global_plugins.push(PluginConfigWrapper {
            plugin_type: "matrix".to_string(),
            parameters: json!({
                "label": "home_cinema_bass_management",
                "input_channel_map": [0],
                "output_channel_map": [1],
                "matrix": [1.0],
            }),
        });

        for format in [
            ExportFormat::CamillaDsp,
            ExportFormat::EqualizerApo,
            ExportFormat::EasyEffects,
            ExportFormat::Wavelet,
            ExportFormat::PipeWire,
            ExportFormat::RoonDsp,
        ] {
            let err = external_export_supported(&output, format).unwrap_err();
            assert!(
                err.to_string()
                    .contains("cannot represent routed home-cinema bass management safely"),
                "unexpected error for {format:?}: {err}"
            );
        }
    }

    #[test]
    fn package_convolution_sidecars_copies_and_rewrites_relative_paths() {
        let source_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();
        std::fs::write(source_dir.path().join("L_fir_96000hz.wav"), b"wav").unwrap();

        let mut output = make_test_output();
        output
            .channels
            .get_mut("left")
            .unwrap()
            .plugins
            .push(PluginConfigWrapper {
                plugin_type: "convolution".to_string(),
                parameters: json!({"ir_file": "L_fir_96000hz.wav"}),
            });

        let packaged =
            package_convolution_sidecars(&output, source_dir.path(), dest_dir.path()).unwrap();

        assert_eq!(
            std::fs::read(dest_dir.path().join("L_fir_96000hz.wav")).unwrap(),
            b"wav"
        );
        let ir_file = packaged.channels["left"]
            .plugins
            .iter()
            .find(|plugin| plugin.plugin_type == "convolution")
            .unwrap()
            .parameters
            .get("ir_file")
            .and_then(|value| value.as_str())
            .unwrap();
        assert_eq!(ir_file, "L_fir_96000hz.wav");
    }

    #[test]
    fn package_convolution_sidecars_avoids_destination_collisions() {
        let source_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();
        std::fs::write(source_dir.path().join("L_fir_96000hz.wav"), b"new").unwrap();
        std::fs::write(dest_dir.path().join("L_fir_96000hz.wav"), b"old").unwrap();

        let mut output = make_test_output();
        output
            .channels
            .get_mut("left")
            .unwrap()
            .plugins
            .push(PluginConfigWrapper {
                plugin_type: "convolution".to_string(),
                parameters: json!({"ir_file": "L_fir_96000hz.wav"}),
            });

        let packaged =
            package_convolution_sidecars(&output, source_dir.path(), dest_dir.path()).unwrap();

        assert_eq!(
            std::fs::read(dest_dir.path().join("L_fir_96000hz_002.wav")).unwrap(),
            b"new"
        );
        let ir_file = packaged.channels["left"]
            .plugins
            .iter()
            .find(|plugin| plugin.plugin_type == "convolution")
            .unwrap()
            .parameters
            .get("ir_file")
            .and_then(|value| value.as_str())
            .unwrap();
        assert_eq!(ir_file, "L_fir_96000hz_002.wav");
    }

    #[test]
    fn export_with_convolution_sidecars_uses_selected_sample_rate() {
        let source_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();
        std::fs::write(source_dir.path().join("L_fir_96000hz.wav"), b"wav").unwrap();

        let mut output = make_test_output();
        output
            .channels
            .get_mut("left")
            .unwrap()
            .plugins
            .push(PluginConfigWrapper {
                plugin_type: "convolution".to_string(),
                parameters: json!({"ir_file": "L_fir_96000hz.wav"}),
            });

        let export_path = dest_dir.path().join("room_eq_cdsp.yaml");
        export_dsp_chain_with_convolution_sidecars(
            &output,
            ExportFormat::CamillaDsp,
            &export_path,
            96_000.0,
            source_dir.path(),
        )
        .unwrap();

        let yaml = std::fs::read_to_string(&export_path).unwrap();
        assert!(yaml.contains("samplerate: 96000"));
        assert!(yaml.contains("filename: L_fir_96000hz.wav"));
        assert!(dest_dir.path().join("L_fir_96000hz.wav").is_file());
    }

    #[test]
    fn test_extract_eq_filters() {
        let plugins = vec![PluginConfigWrapper {
            plugin_type: "eq".to_string(),
            parameters: json!({
                "filters": [
                    {"filter_type": "peak", "freq": 100.0, "q": 2.0, "db_gain": -5.0},
                    {"filter_type": "highshelf", "freq": 8000.0, "q": 0.7, "db_gain": -2.0},
                ]
            }),
        }];
        let filters = extract_eq_filters(&plugins);
        assert_eq!(filters.len(), 2);
        assert_eq!(filters[0].filter_type, "peak");
        assert_eq!(filters[0].freq, 100.0);
        assert_eq!(filters[1].filter_type, "highshelf");
    }

    #[test]
    fn test_extract_gain_db() {
        let plugins = vec![
            PluginConfigWrapper {
                plugin_type: "gain".to_string(),
                parameters: json!({"gain_db": -2.5}),
            },
            PluginConfigWrapper {
                plugin_type: "gain".to_string(),
                parameters: json!({"gain_db": 1.0}),
            },
        ];
        let gain = extract_gain_db(&plugins);
        assert!((gain - (-1.5)).abs() < 0.01);
    }

    #[test]
    fn test_extract_delay_ms() {
        let plugins = vec![PluginConfigWrapper {
            plugin_type: "delay".to_string(),
            parameters: json!({"delay_ms": 3.5}),
        }];
        assert_eq!(extract_delay_ms(&plugins), Some(3.5));

        let empty: Vec<PluginConfigWrapper> = vec![];
        assert_eq!(extract_delay_ms(&empty), None);
    }

    #[test]
    fn test_export_camilladsp() {
        let output = make_test_output();
        let result = export_camilladsp(&output, 48000.0).unwrap();

        assert!(result.contains("samplerate: 48000"));
        assert!(result.contains("left_gain:"));
        assert!(result.contains("left_delay:"));
        assert!(result.contains("left_peq_0:"));
        assert!(result.contains("left_peq_1:"));
        assert!(result.contains("left_peq_2:"));
        assert!(result.contains("right_gain:"));
        assert!(result.contains("right_peq_0:"));
        assert!(result.contains("type: Biquad"));
        assert!(result.contains("type: Peaking"));
        assert!(result.contains("type: Highshelf"));
        assert!(result.contains("type: Gain"));
        assert!(result.contains("type: Delay"));
        assert!(result.contains("unit: ms"));
        assert!(result.contains("pipeline:"));
    }

    #[test]
    fn test_camilladsp_pipeline_uses_gui_friendly_steps() {
        let output = make_test_output();
        let result = export_camilladsp(&output, 48000.0).unwrap();

        assert!(
            result.contains(
                "pipeline:\n- bypassed: null\n  channels:\n  - 0\n  names:\n  - left_gain"
            ),
            "Expected pipeline entries to start with bypassed null, got:\n{result}"
        );
        assert!(
            result.contains("  type: Filter\n- bypassed: null"),
            "Expected type line inside the pipeline step, got:\n{result}"
        );
        assert!(
            !result.contains("  - type: Filter"),
            "Pipeline step should not start with a dashed type line"
        );
        assert!(result.contains("  - left_delay"));
        assert!(result.contains("  - left_peq_0"));
        assert!(result.contains("  - right_gain"));
    }

    #[test]
    fn test_export_equalizer_apo() {
        let output = make_test_output();
        let result = export_equalizer_apo(&output).unwrap();

        assert!(result.contains("Channel: L"));
        assert!(result.contains("Channel: R"));
        assert!(result.contains("Preamp: -2.5 dB"));
        assert!(result.contains("Delay: 1.500 ms"));
        assert!(result.contains("Filter  1: ON PK Fc 100 Hz Gain -5.00 dB Q 2.0000"));
        assert!(result.contains("Filter  3: ON HSC Fc 8000 Hz Gain -2.00 dB Q 0.7000"));
        assert!(result.contains("Filter  1: ON PK Fc 200 Hz Gain -3.00 dB Q 1.0000"));
        assert!(result.contains("Filter  2: ON LSC Fc 80 Hz Gain +4.00 dB Q 0.7100"));
    }

    #[test]
    fn test_export_easyeffects() {
        let output = make_test_output();
        let result = export_easyeffects(&output).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let eq = &parsed["output"]["equalizer#0"];
        assert_eq!(eq["num-bands"].as_u64().unwrap(), 5);
        assert!(eq["left"]["band0"]["frequency"].as_f64().unwrap() > 0.0);

        // Check filter types
        let band0_type = eq["left"]["band0"]["type"].as_str().unwrap();
        assert_eq!(band0_type, "Bell");
    }

    #[test]
    fn test_export_wavelet() {
        let output = make_test_output();
        let result = export_wavelet(&output, 48000.0).unwrap();

        assert!(result.contains("GraphicEQ:"));
        // Should have 9 frequency/gain pairs
        let line = result
            .lines()
            .find(|l| l.starts_with("GraphicEQ:"))
            .unwrap();
        let parts: Vec<&str> = line.trim_start_matches("GraphicEQ:").split(';').collect();
        assert_eq!(parts.len(), 9);
    }

    #[test]
    fn test_export_pipewire() {
        let output = make_test_output();
        let result = export_pipewire(&output, 48000.0).unwrap();

        assert!(result.contains("libpipewire-module-filter-chain"));
        assert!(result.contains("bq_peaking"));
        assert!(result.contains("bq_highshelf"));
        assert!(result.contains("filter.graph"));
        assert!(result.contains("nodes ="));
        assert!(result.contains("links ="));
        assert!(result.contains("audio.channels = 2"));
        assert!(result.contains("\"FL\""));
        assert!(result.contains("\"FR\""));
    }

    #[test]
    fn test_export_format_extensions() {
        assert_eq!(ExportFormat::CamillaDsp.default_extension(), "yaml");
        assert_eq!(ExportFormat::EqualizerApo.default_extension(), "txt");
        assert_eq!(ExportFormat::EasyEffects.default_extension(), "json");
        assert_eq!(ExportFormat::Wavelet.default_extension(), "txt");
        assert_eq!(ExportFormat::PipeWire.default_extension(), "conf");
        assert_eq!(ExportFormat::RoonDsp.default_extension(), "json");
        assert_eq!(
            ExportFormat::CamillaDsp.default_file_name(),
            "room_eq_cdsp.yaml"
        );
        assert_eq!(
            ExportFormat::CamillaDsp.default_export_path(Path::new("out/room_eq.json")),
            PathBuf::from("out/room_eq_cdsp.yaml")
        );
        assert_eq!(
            ExportFormat::EqualizerApo.default_export_path(Path::new("out/room_eq.json")),
            PathBuf::from("out/room_eq.txt")
        );
    }

    #[test]
    fn test_export_roon() {
        let output = make_test_output();
        let result = export_roon(&output).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let channels = &parsed["channels"];

        // Left channel
        let left = &channels["left"];
        assert!(left["headroom_gain_db"].as_f64().unwrap() < 0.0);
        assert!((left["delay_ms"].as_f64().unwrap() - 1.5).abs() < 0.01);

        let left_bands = left["parametric_eq"]["bands"].as_array().unwrap();
        assert_eq!(left_bands.len(), 3);
        assert_eq!(left_bands[0]["type"].as_str().unwrap(), "Peak/Dip");
        assert_eq!(left_bands[0]["frequency"].as_f64().unwrap(), 100.0);
        assert_eq!(left_bands[2]["type"].as_str().unwrap(), "High Shelf");

        // Right channel
        let right = &channels["right"];
        assert!(right["headroom_gain_db"].as_f64().unwrap() < 0.0);
        assert!(right.get("delay_ms").is_none()); // no delay on right

        let right_bands = right["parametric_eq"]["bands"].as_array().unwrap();
        assert_eq!(right_bands.len(), 2);
        assert_eq!(right_bands[1]["type"].as_str().unwrap(), "Low Shelf");
        assert!(right_bands[0]["enabled"].as_bool().unwrap());
    }

    #[test]
    fn test_camilladsp_uses_second_order_filters() {
        // Bug: lowpass/highpass were mapped to LowpassFO/HighpassFO (first-order)
        // but roomeq biquads are second-order
        let mut channels = HashMap::new();
        channels.insert(
            "left".to_string(),
            ChannelDspChain {
                channel: "left".to_string(),
                plugins: vec![PluginConfigWrapper {
                    plugin_type: "eq".to_string(),
                    parameters: json!({
                        "filters": [
                            {"filter_type": "highpass", "freq": 80.0, "q": 0.71, "db_gain": 0.0},
                            {"filter_type": "lowpass", "freq": 16000.0, "q": 0.71, "db_gain": 0.0},
                        ]
                    }),
                }],
                drivers: None,
                initial_curve: None,
                final_curve: None,
                eq_response: None,
                target_curve: None,
                pre_ir: None,
                post_ir: None,
                fir_temporal_masking: None,
                direct_early_late_correction: None,
            },
        );
        let output = DspChainOutput {
            version: "1.3.0".to_string(),
            global_plugins: Vec::new(),
            channels,
            metadata: None,
        };
        let result = export_camilladsp(&output, 48000.0).unwrap();
        // Must be second-order Highpass/Lowpass, NOT HighpassFO/LowpassFO
        assert!(
            result.contains("type: Highpass"),
            "Expected second-order Highpass, got:\n{result}"
        );
        assert!(
            result.contains("type: Lowpass"),
            "Expected second-order Lowpass, got:\n{result}"
        );
        assert!(
            !result.contains("FO"),
            "Should not contain first-order filter types"
        );
    }

    #[test]
    fn test_camilladsp_no_duplicate_yaml_keys() {
        // Bug: multiple gain plugins in same list both named {prefix}_gain
        let mut channels = HashMap::new();
        channels.insert(
            "left".to_string(),
            ChannelDspChain {
                channel: "left".to_string(),
                plugins: vec![
                    PluginConfigWrapper {
                        plugin_type: "gain".to_string(),
                        parameters: json!({"gain_db": -3.0}),
                    },
                    PluginConfigWrapper {
                        plugin_type: "gain".to_string(),
                        parameters: json!({"gain_db": -1.0, "invert": true}),
                    },
                ],
                drivers: None,
                initial_curve: None,
                final_curve: None,
                eq_response: None,
                target_curve: None,
                pre_ir: None,
                post_ir: None,
                fir_temporal_masking: None,
                direct_early_late_correction: None,
            },
        );
        let output = DspChainOutput {
            version: "1.3.0".to_string(),
            global_plugins: Vec::new(),
            channels,
            metadata: None,
        };
        let result = export_camilladsp(&output, 48000.0).unwrap();
        // First gain: "left_gain:", second: "left_gain_1:"
        assert!(result.contains("left_gain:"));
        assert!(result.contains("left_gain_1:"));
    }

    #[test]
    fn test_easyeffects_uses_min_gain() {
        // Bug: was using largest absolute gain which could pick positive gain
        let mut channels = HashMap::new();
        channels.insert(
            "left".to_string(),
            ChannelDspChain {
                channel: "left".to_string(),
                plugins: vec![PluginConfigWrapper {
                    plugin_type: "gain".to_string(),
                    parameters: json!({"gain_db": -5.0}),
                }],
                drivers: None,
                initial_curve: None,
                final_curve: None,
                eq_response: None,
                target_curve: None,
                pre_ir: None,
                post_ir: None,
                fir_temporal_masking: None,
                direct_early_late_correction: None,
            },
        );
        channels.insert(
            "right".to_string(),
            ChannelDspChain {
                channel: "right".to_string(),
                plugins: vec![PluginConfigWrapper {
                    plugin_type: "gain".to_string(),
                    parameters: json!({"gain_db": 3.0}),
                }],
                drivers: None,
                initial_curve: None,
                final_curve: None,
                eq_response: None,
                target_curve: None,
                pre_ir: None,
                post_ir: None,
                fir_temporal_masking: None,
                direct_early_late_correction: None,
            },
        );
        let output = DspChainOutput {
            version: "1.3.0".to_string(),
            global_plugins: Vec::new(),
            channels,
            metadata: None,
        };
        let result = export_easyeffects(&output).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        let input_gain = parsed["output"]["equalizer#0"]["input-gain"]
            .as_f64()
            .unwrap();
        // Should use -5.0 (most negative) to prevent clipping, not +3.0
        assert!(
            (input_gain - (-5.0)).abs() < 0.01,
            "Expected -5.0 gain, got {input_gain}"
        );
    }

    #[test]
    fn test_unknown_channels_sort_alphabetically() {
        // Bug: unknown channels all mapped to index 0, non-deterministic ordering
        let mut channels = HashMap::new();
        for name in &["sub2", "sub0", "sub1"] {
            channels.insert(
                name.to_string(),
                ChannelDspChain {
                    channel: name.to_string(),
                    plugins: vec![],
                    drivers: None,
                    initial_curve: None,
                    final_curve: None,
                    eq_response: None,
                    target_curve: None,
                    pre_ir: None,
                    post_ir: None,
                    fir_temporal_masking: None,
                    direct_early_late_correction: None,
                },
            );
        }
        let output = DspChainOutput {
            version: "1.3.0".to_string(),
            global_plugins: Vec::new(),
            channels,
            metadata: None,
        };
        let sorted = sorted_channels(&output);
        let names: Vec<&str> = sorted.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["sub0", "sub1", "sub2"]);
    }

    #[test]
    fn test_highpassvariableq_mapped_correctly() {
        // Bug: highpassvariableq fell through to Peak
        assert_eq!(
            parse_biquad_filter_type("highpassvariableq").unwrap(),
            BiquadFilterType::HighpassVariableQ
        );
        assert_eq!(camilladsp_filter_type("highpassvariableq"), "Highpass");
        assert_eq!(apo_filter_type("highpassvariableq"), "HP");
        assert_eq!(easyeffects_filter_type("highpassvariableq"), "Hi-pass");
        assert_eq!(
            pipewire_filter_label("highpassvariableq").unwrap(),
            "bq_highpass"
        );
        assert_eq!(roon_filter_type("highpassvariableq"), "High Pass");
    }

    #[test]
    fn test_unknown_biquad_filter_type_is_rejected() {
        let err = parse_biquad_filter_type("lowsehlf").unwrap_err();

        assert!(
            err.to_string().contains("Unsupported biquad filter type"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_wavelet_rejects_unknown_filter_type() {
        let output = make_single_filter_output("lowsehlf", 3.0);

        let err = export_wavelet(&output, 48_000.0).unwrap_err();

        assert!(
            err.to_string().contains("Unsupported biquad filter type"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_pipewire_rejects_unknown_filter_type() {
        let output = make_single_filter_output("lowsehlf", 3.0);

        let err = export_pipewire(&output, 48_000.0).unwrap_err();

        assert!(
            err.to_string()
                .contains("Unsupported PipeWire biquad filter type"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_pipewire_highpassvariableq_omits_gain_control() {
        let output = make_single_filter_output("highpassvariableq", -6.0);

        let conf = export_pipewire(&output, 48_000.0).unwrap();

        assert!(conf.contains("label = bq_highpass"));
        assert!(
            conf.contains("control = { \"Freq\" = 80.0  \"Q\" = 0.7070 }"),
            "highpassvariableq should emit only Freq/Q controls:\n{conf}"
        );
        assert!(
            !conf.contains("\"Gain\" = -6.00"),
            "PipeWire highpassvariableq must not emit unsupported Gain control:\n{conf}"
        );
    }

    #[test]
    fn test_export_with_drivers() {
        let mut channels = HashMap::new();
        channels.insert(
            "left".to_string(),
            ChannelDspChain {
                channel: "left".to_string(),
                plugins: vec![PluginConfigWrapper {
                    plugin_type: "eq".to_string(),
                    parameters: json!({
                        "filters": [
                            {"filter_type": "peak", "freq": 500.0, "q": 1.0, "db_gain": -2.0},
                        ]
                    }),
                }],
                drivers: Some(vec![
                    DriverDspChain {
                        name: "woofer".to_string(),
                        index: 0,
                        plugins: vec![
                            PluginConfigWrapper {
                                plugin_type: "gain".to_string(),
                                parameters: json!({"gain_db": -3.0}),
                            },
                            PluginConfigWrapper {
                                plugin_type: "delay".to_string(),
                                parameters: json!({"delay_ms": 2.0}),
                            },
                        ],
                        initial_curve: None,
                    },
                    DriverDspChain {
                        name: "tweeter".to_string(),
                        index: 1,
                        plugins: vec![PluginConfigWrapper {
                            plugin_type: "gain".to_string(),
                            parameters: json!({"gain_db": 0.0, "invert": true}),
                        }],
                        initial_curve: None,
                    },
                ]),
                initial_curve: None,
                final_curve: None,
                eq_response: None,
                target_curve: None,
                pre_ir: None,
                post_ir: None,
                fir_temporal_masking: None,
                direct_early_late_correction: None,
            },
        );

        let output = DspChainOutput {
            version: "1.3.0".to_string(),
            global_plugins: Vec::new(),
            channels,
            metadata: None,
        };

        // CamillaDSP should include driver filters
        let cdsp = export_camilladsp(&output, 48000.0).unwrap();
        assert!(cdsp.contains("left_woofer_gain:"));
        assert!(cdsp.contains("left_woofer_delay:"));
        assert!(cdsp.contains("left_tweeter_gain:"));
        assert!(cdsp.contains("inverted: true"));

        // APO should include driver gain and delay
        let apo = export_equalizer_apo(&output).unwrap();
        assert!(apo.contains("Preamp: -3.0 dB"));
        assert!(apo.contains("Delay: 2.000 ms"));
    }
}
