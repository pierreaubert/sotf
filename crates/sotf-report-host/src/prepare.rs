//! Native preparation of shared AutoEQ report inputs.
//!
//! Ports the file-backed part of AutoEQ's Python loader contract: the
//! optimizer result graph stays authoritative, while curve/IR/blob fields
//! that slim results keep as sidecar files in the artifact directory are
//! re-injected from `measurements_index.json`. No Python runtime is
//! involved; the prepared DTO is consumed by
//! `autoeq_report_wasm::report_dataset_payload::build_dataset_report`.

// Rust guideline compliant 2026-02-21

use anyhow::Context;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

use autoeq_report_wasm::PreparedReportData;
use autoeq_report_wasm::report_dataset::ReportDatasetOptions;

const CURVE_FIELDS: [&str; 4] = [
    "initial_curve",
    "final_curve",
    "eq_response",
    "target_curve",
];
const IR_FIELDS: [&str; 2] = ["pre_ir", "post_ir"];
const BLOB_FIELDS: [&str; 4] = [
    "early_late_curves",
    "waterfall",
    "resonance_decays",
    "wavelet",
];
const IR64_MAGIC: &[u8; 8] = b"AEQIR64\0";

/// Build the prepared report DTO and dataset options for one result.
///
/// `artifact_root`, when present, is the optimizer output directory holding
/// `measurements_index.json` and its referenced sidecar files. Fields the
/// graph already embeds are never overwritten.
///
/// # Errors
/// Returns an error when the artifact root is not a directory or the
/// prepared DTO fails upstream validation.
pub fn prepare_report(
    result: &Value,
    artifact_root: Option<&Path>,
) -> anyhow::Result<(PreparedReportData, ReportDatasetOptions)> {
    let mut graph = result.clone();
    let mut measurement_index: Option<Value> = None;
    let mut symmetric_pairs: Option<Value> = None;
    if let Some(root) = artifact_root {
        let root = root
            .canonicalize()
            .context("Resolving AutoEQ report artifacts")?;
        anyhow::ensure!(
            root.is_dir(),
            "AutoEQ report artifact root is not a directory"
        );
        (measurement_index, symmetric_pairs) = apply_measurement_overlay(&mut graph, &root);
    }
    let prepared = PreparedReportData::new(
        graph,
        Some(report_provenance(result)),
        measurement_index,
        symmetric_pairs,
        target_snapshot(result),
        None,
    )
    .map_err(anyhow::Error::msg)?;
    Ok((prepared, ReportDatasetOptions::default()))
}

fn report_provenance(result: &Value) -> Value {
    let mut versions = Vec::new();
    if let Some(version) = result["version"].as_str() {
        versions.push(json!(version));
    }
    let mut timestamps = Vec::new();
    if let Some(timestamp) = result["metadata"]["timestamp"].as_str() {
        timestamps.push(json!(timestamp));
    }
    json!({"versions": versions, "timestamps": timestamps})
}

fn target_snapshot(result: &Value) -> Value {
    // An embedded target curve in the result metadata is authoritative;
    // anything else (paths into user files) cannot be honored offline.
    let target = &result["metadata"]["target_curve"];
    if target["freq"].is_array() && target["spl"].is_array() {
        return json!({
            "schema_version": 1,
            "state": "available",
            "resolution": "embedded_result",
            "target": target,
        });
    }
    json!({"schema_version": 1, "state": "not_configured", "target": null})
}

/// Re-inject indexed sidecar fields into `graph`; returns the overlay index
/// (channel -> field -> filename) and the symmetric-pairs block, mirroring
/// the Python loader's `measurement_index` attribute.
fn apply_measurement_overlay(graph: &mut Value, assets: &Path) -> (Option<Value>, Option<Value>) {
    let index = read_json_file(&assets.join("measurements_index.json")).unwrap_or(Value::Null);
    let channels_index = index["channels"].as_object();
    let symmetric_pairs = index["symmetric_pairs"]
        .is_object()
        .then(|| index["symmetric_pairs"].clone());
    let mut overlay = Map::new();
    let Some(channels) = graph["channels"].as_object_mut() else {
        return (None, symmetric_pairs);
    };
    for (name, channel) in channels.iter_mut() {
        let entry = channels_index.and_then(|map| map.get(name));
        let mut channel_overlay = Map::new();
        for field in CURVE_FIELDS {
            if !channel[field].is_null() {
                continue;
            }
            let Some((filename, curve)) = read_indexed_curve(assets, entry, field) else {
                continue;
            };
            channel[field] = curve;
            channel_overlay.insert(field.into(), json!(filename));
        }
        for field in IR_FIELDS {
            if !channel[field].is_null() {
                continue;
            }
            let Some(filename) = indexed_filename(entry, field) else {
                continue;
            };
            let Some(path) = bundle_member(assets, &filename) else {
                continue;
            };
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let Some(ir) = read_ir(&bytes) else { continue };
            channel[field] = ir;
            channel_overlay.insert(field.into(), json!(filename));
        }
        for field in BLOB_FIELDS {
            if !channel[field].is_null() {
                continue;
            }
            let Some(filename) = indexed_filename(entry, field) else {
                continue;
            };
            let Some(path) = bundle_member(assets, &filename) else {
                continue;
            };
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            let Ok(blob) = serde_json::from_slice::<Value>(&bytes) else {
                continue;
            };
            channel[field] = blob;
            channel_overlay.insert(field.into(), json!(filename));
        }
        apply_driver_overlay(assets, entry, channel, &mut channel_overlay);
        if !channel_overlay.is_empty() {
            overlay.insert(name.clone(), Value::Object(channel_overlay));
        }
    }
    let measurement_index = if overlay.is_empty() && index["deployed_source_curves"].is_null() {
        None
    } else {
        let mut value = Map::new();
        value.insert("channels".into(), Value::Object(overlay));
        if !index["deployed_source_curves"].is_null() {
            value.insert(
                "deployed_source_curves".into(),
                index["deployed_source_curves"].clone(),
            );
        }
        Some(Value::Object(value))
    };
    (measurement_index, symmetric_pairs)
}

/// Driver-level sidecars: `measured_acoustics` blobs and initial curves.
fn apply_driver_overlay(
    assets: &Path,
    entry: Option<&Value>,
    channel: &mut Value,
    channel_overlay: &mut Map<String, Value>,
) {
    let Some(drivers) = channel["drivers"].as_array_mut() else {
        return;
    };
    for (driver_index, driver) in drivers.iter_mut().enumerate() {
        if driver["measured_acoustics"].is_null()
            && let Some((kind, filename)) = indexed_kind(entry, driver_index, "_measured_acoustics")
            && let Some(path) = bundle_member(assets, &filename)
            && let Ok(bytes) = std::fs::read(&path)
            && let Ok(blob) = serde_json::from_slice::<Value>(&bytes)
        {
            driver["measured_acoustics"] = blob;
            channel_overlay.insert(kind, json!(filename));
        }
        if driver["initial_curve"].is_null()
            && let Some((kind, filename)) = indexed_kind(entry, driver_index, "_initial_curve")
            && let Some(curve) = read_indexed_curve_kind(assets, entry, &kind, &filename)
        {
            driver["initial_curve"] = curve;
            channel_overlay.insert(kind, json!(filename));
        }
    }
}

fn indexed_filename(entry: Option<&Value>, field: &str) -> Option<String> {
    entry
        .and_then(|entry| entry[field].as_str())
        .map(str::to_owned)
}

fn indexed_kind(
    entry: Option<&Value>,
    driver_index: usize,
    suffix: &str,
) -> Option<(String, String)> {
    let prefix = format!("driver{driver_index}_");
    entry?.as_object()?.iter().find_map(|(kind, value)| {
        if kind.starts_with(&prefix) && kind.ends_with(suffix) {
            value
                .as_str()
                .map(|filename| (kind.clone(), filename.to_owned()))
        } else {
            None
        }
    })
}

/// Read one indexed curve for `field`, honoring its optional precomputed
/// metadata JSON (`{field}_metadata`) exactly like the Python loader.
fn read_indexed_curve(
    assets: &Path,
    entry: Option<&Value>,
    field: &str,
) -> Option<(String, Value)> {
    let filename = indexed_filename(entry, field)?;
    let curve = read_indexed_curve_kind(assets, entry, field, &filename)?;
    Some((filename, curve))
}

fn read_indexed_curve_kind(
    assets: &Path,
    entry: Option<&Value>,
    kind: &str,
    filename: &str,
) -> Option<Value> {
    if let Some(metadata_name) = indexed_filename(entry, &format!("{kind}_metadata"))
        && let Some(path) = bundle_member(assets, &metadata_name)
        && let Ok(bytes) = std::fs::read(&path)
        && let Ok(curve) = serde_json::from_slice::<Value>(&bytes)
        && curve["freq"].is_array()
        && curve["spl"].is_array()
    {
        return Some(curve);
    }
    let path = bundle_member(assets, filename)?;
    let text = std::fs::read_to_string(path).ok()?;
    read_curve_csv(&text)
}

fn read_curve_csv(text: &str) -> Option<Value> {
    let mut lines = text.lines();
    let headers: Vec<&str> = lines.next()?.split(',').map(str::trim).collect();
    let freq_at = headers.iter().position(|name| *name == "freq")?;
    let spl_at = headers.iter().position(|name| *name == "spl")?;
    let phase_at = headers.iter().position(|name| *name == "phase");
    let (mut freq, mut spl, mut phase) = (Vec::new(), Vec::new(), Vec::new());
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let cells: Vec<&str> = line.split(',').collect();
        freq.push(cells.get(freq_at)?.trim().parse::<f64>().ok()?);
        spl.push(cells.get(spl_at)?.trim().parse::<f64>().ok()?);
        if let Some(phase_at) = phase_at {
            let raw = cells.get(phase_at).map_or("", |cell| cell.trim());
            phase.push(match raw {
                "" | "nan" | "NaN" | "NAN" => Value::Null,
                _ => raw
                    .parse::<f64>()
                    .ok()
                    .map(json_number)
                    .unwrap_or(Value::Null),
            });
        }
    }
    let mut curve = Map::new();
    curve.insert("freq".into(), json!(freq));
    curve.insert("spl".into(), json!(spl));
    if phase_at.is_some() {
        curve.insert("phase".into(), Value::Array(phase));
    }
    Some(Value::Object(curve))
}

// `Iterator::array_chunks` is not stable in this toolchain; `chunks_exact`
// with a literal pair size is the stable equivalent here.
#[allow(clippy::chunks_exact_to_as_chunks)]
fn read_ir(bytes: &[u8]) -> Option<Value> {
    if bytes.starts_with(IR64_MAGIC) {
        if bytes.len() < 16 {
            return None;
        }
        let count = u64::from_le_bytes(bytes[8..16].try_into().ok()?) as usize;
        if bytes.len() != 16 + count * 16 {
            return None;
        }
        let (mut time_ms, mut amplitude) = (Vec::with_capacity(count), Vec::with_capacity(count));
        for pair in bytes[16..].chunks_exact(16) {
            time_ms.push(f64::from_le_bytes(pair[..8].try_into().ok()?));
            amplitude.push(f64::from_le_bytes(pair[8..].try_into().ok()?));
        }
        return Some(json!({"time_ms": time_ms, "amplitude": amplitude}));
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let mut lines = text.lines();
    let headers: Vec<&str> = lines.next()?.split(',').map(str::trim).collect();
    let time_at = headers.iter().position(|name| *name == "time_ms")?;
    let amp_at = headers.iter().position(|name| *name == "amplitude")?;
    let (mut time_ms, mut amplitude) = (Vec::new(), Vec::new());
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let cells: Vec<&str> = line.split(',').collect();
        time_ms.push(cells.get(time_at)?.trim().parse::<f64>().ok()?);
        amplitude.push(cells.get(amp_at)?.trim().parse::<f64>().ok()?);
    }
    if time_ms
        .iter()
        .chain(&amplitude)
        .any(|value| !value.is_finite())
    {
        return None;
    }
    Some(json!({"time_ms": time_ms, "amplitude": amplitude}))
}

fn json_number(value: f64) -> Value {
    serde_json::Number::from_f64(value).map_or(Value::Null, Value::Number)
}

fn read_json_file(path: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// Resolve a plain file name inside the artifact directory; nested or
/// traversing names are rejected like the Python loader's bundle guard.
fn bundle_member(assets: &Path, filename: &str) -> Option<PathBuf> {
    if filename.is_empty()
        || filename.contains(['/', '\\'])
        || filename == ".."
        || Path::new(filename).is_absolute()
    {
        return None;
    }
    let path = assets.join(filename);
    path.is_file().then_some(path)
}
