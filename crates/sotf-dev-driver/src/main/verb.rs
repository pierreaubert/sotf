use super::ctx::post_dev_json;
use super::misc::focus_action_name;
use super::misc::split2;
use super::misc::urlencode;
use super::parse::parse_compare;
use super::parse::parse_dev_response;
use super::types::Ctx;
use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};

pub(super) fn verb_action(rest: &str, ctx: &Ctx) -> Result<()> {
    let (name, payload_raw) = split2(rest);
    if name.is_empty() {
        bail!("action verb needs a name");
    }
    let payload: Option<Value> = if payload_raw.trim().is_empty() {
        None
    } else {
        Some(serde_json::from_str(payload_raw.trim()).context("payload is not valid JSON")?)
    };
    let body = serde_json::json!({ "name": name, "payload": payload });
    post_dev_json(ctx, "/action", &body, &format!("action `{name}`"))?;
    Ok(())
}

pub(super) fn verb_query(rest: &str, ctx: &Ctx) -> Result<Value> {
    let path = rest.trim();
    if path.is_empty() {
        bail!("query verb needs a path");
    }
    let url = format!("{}/query?path={}", ctx.base, urlencode(path));
    let resp = ctx.client.get(url).send()?;
    let json = parse_dev_response(resp, &format!("query `{path}`"))?;
    json.get("value")
        .cloned()
        .ok_or_else(|| anyhow!("server returned no `value`"))
}

pub(super) fn verb_assert(rest: &str, ctx: &Ctx) -> Result<()> {
    let cmp = parse_compare(rest)?;
    let actual = verb_query(&cmp.path, ctx)?;
    if !cmp.matches(&actual) {
        bail!(
            "assertion failed: {} {} {} (got {})",
            cmp.path,
            cmp.op.as_str(),
            cmp.expected_text,
            actual
        );
    }
    if ctx.verbose {
        println!("    -> ok ({actual})");
    }
    Ok(())
}

/// Compare a QA screenshot against a baseline using mean absolute RGBA delta.
///
/// On a mismatch, write expected, actual, difference, and side-by-side
/// comparison PNGs below the isolated QA directory for CI diagnosis.
/// Syntax: `assert_snapshot <name> <baseline.png> [tolerance]`.
pub(super) fn verb_assert_snapshot(rest: &str, ctx: &Ctx) -> Result<()> {
    let mut parts = rest.split_whitespace();
    let name = parts
        .next()
        .ok_or_else(|| anyhow!("assert_snapshot needs `<name> <baseline.png> [tolerance]`"))?;
    let baseline = parts
        .next()
        .ok_or_else(|| anyhow!("assert_snapshot needs `<name> <baseline.png> [tolerance]`"))?;
    let tolerance = parts
        .next()
        .map(str::parse::<f64>)
        .transpose()
        .context("snapshot tolerance must be a number")?
        .unwrap_or(0.0);
    if parts.next().is_some() {
        bail!("assert_snapshot accepts only a name, baseline path, and optional tolerance");
    }
    if !tolerance.is_finite() || !(0.0..=1.0).contains(&tolerance) {
        bail!("snapshot tolerance must be between 0 and 1");
    }

    let actual = qa_screenshot_path(ctx, name)?;
    let actual_image = image::open(&actual)
        .with_context(|| format!("opening actual screenshot {}", actual.display()))?
        .to_rgba8();
    let baseline = std::path::PathBuf::from(baseline);
    let baseline_image = image::open(&baseline)
        .with_context(|| format!("opening snapshot baseline {}", baseline.display()))?
        .to_rgba8();
    let artifact_directory = qa_snapshot_artifact_directory(ctx)?;
    if actual_image.dimensions() != baseline_image.dimensions() {
        let artifacts = write_dimension_mismatch_artifacts(
            &actual_image,
            &baseline_image,
            &artifact_directory,
            name,
        )?;
        bail!(
            "snapshot dimensions differ: actual {:?}, baseline {:?}; artifacts={}",
            actual_image.dimensions(),
            baseline_image.dimensions(),
            artifacts.display(),
        );
    }
    let delta = mean_pixel_delta(actual_image.as_raw(), baseline_image.as_raw());
    if delta > tolerance {
        let artifacts =
            write_snapshot_artifacts(&actual_image, &baseline_image, &artifact_directory, name)?;
        bail!(
            "snapshot delta {delta:.6} exceeds tolerance {tolerance:.6}: actual={}, baseline={}, artifacts={}",
            actual.display(),
            baseline.display(),
            artifacts.display(),
        );
    }
    if ctx.verbose {
        println!("    -> snapshot delta {delta:.6}");
    }
    Ok(())
}

/// Require a rendered accessibility node with a role and label substring.
/// Syntax: `assert_accessible <role> <label substring>`.
pub(super) fn verb_assert_accessible(rest: &str, ctx: &Ctx) -> Result<()> {
    let (role, expected_label) = split2(rest);
    let expected_label = expected_label.trim();
    if role.is_empty() || expected_label.is_empty() {
        bail!("assert_accessible needs `<role> <label substring>`");
    }
    let response = ctx
        .client
        .get(format!("{}/accessibility", ctx.base))
        .send()?;
    let json = parse_dev_response(response, "accessibility")?;
    let nodes = json
        .get("value")
        .and_then(|value| value.get("nodes"))
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("accessibility response contains no nodes"))?;
    let found = nodes.iter().any(|node| {
        node.get("role")
            .and_then(Value::as_str)
            .is_some_and(|actual_role| actual_role.eq_ignore_ascii_case(role))
            && node
                .get("label")
                .and_then(Value::as_str)
                .is_some_and(|actual_label| actual_label.contains(expected_label))
    });
    if !found {
        bail!("no accessible node with role `{role}` and label containing `{expected_label}`");
    }
    Ok(())
}

/// Require a named accessibility node to expose no clear-text value.
/// Syntax: `assert_accessible_masked <role> <label substring>`.
pub(super) fn verb_assert_accessible_masked(rest: &str, ctx: &Ctx) -> Result<()> {
    let (role, expected_label) = split2(rest);
    let expected_label = expected_label.trim();
    if role.is_empty() || expected_label.is_empty() {
        bail!("assert_accessible_masked needs `<role> <label substring>`");
    }

    let response = ctx
        .client
        .get(format!("{}/accessibility", ctx.base))
        .send()?;
    let json = parse_dev_response(response, "accessibility")?;
    let nodes = json
        .get("value")
        .and_then(|value| value.get("nodes"))
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("accessibility response contains no nodes"))?;
    let node = nodes.iter().find(|node| {
        node.get("role")
            .and_then(Value::as_str)
            .is_some_and(|actual_role| actual_role.eq_ignore_ascii_case(role))
            && node
                .get("label")
                .and_then(Value::as_str)
                .is_some_and(|actual_label| actual_label.contains(expected_label))
    });
    let Some(node) = node else {
        bail!("no accessible node with role `{role}` label containing `{expected_label}`");
    };
    let exposed_text = node
        .get("value")
        .and_then(|value| value.get("text"))
        .and_then(Value::as_str);
    if exposed_text.is_some_and(|text| {
        !text.is_empty()
            && !text
                .chars()
                .all(|character| matches!(character, '*' | '•' | '●'))
    }) {
        bail!(
            "accessible node with role `{role}` label containing `{expected_label}` exposes clear text"
        );
    }
    Ok(())
}

/// Match a role and label substring in an accessibility node snapshot.
pub(super) fn accessibility_node_matches(
    nodes: &[Value],
    role: &str,
    expected_label: &str,
) -> bool {
    nodes.iter().any(|node| {
        node.get("role")
            .and_then(Value::as_str)
            .is_some_and(|actual_role| actual_role.eq_ignore_ascii_case(role))
            && node
                .get("label")
                .and_then(Value::as_str)
                .is_some_and(|actual_label| actual_label.contains(expected_label))
    })
}

/// Require that no currently rendered accessibility node has the role and
/// label substring. Syntax: `assert_inaccessible <role> <label substring>`.
pub(super) fn verb_assert_inaccessible(rest: &str, ctx: &Ctx) -> Result<()> {
    let (role, expected_label) = split2(rest);
    let expected_label = expected_label.trim();
    if role.is_empty() || expected_label.is_empty() {
        bail!("assert_inaccessible needs `<role> <label substring>`");
    }
    let response = ctx
        .client
        .get(format!("{}/accessibility", ctx.base))
        .send()?;
    let json = parse_dev_response(response, "accessibility")?;
    let nodes = json
        .get("value")
        .and_then(|value| value.get("nodes"))
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("accessibility response contains no nodes"))?;
    if accessibility_node_matches(nodes, role, expected_label) {
        bail!(
            "unexpected rendered accessibility node role={role:?} label containing {expected_label:?}"
        );
    }
    Ok(())
}

/// Require that the rendered accessibility node for an element ID currently
/// owns keyboard focus. Syntax: `assert_focused <element id>`.
pub(super) fn verb_assert_focused(rest: &str, ctx: &Ctx) -> Result<()> {
    let expected = rest.trim();
    if expected.is_empty() {
        bail!("assert_focused needs an element id");
    }
    let response = ctx
        .client
        .get(format!("{}/accessibility", ctx.base))
        .send()?;
    let json = parse_dev_response(response, "accessibility")?;
    let nodes = json
        .get("value")
        .and_then(|value| value.get("nodes"))
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("accessibility response contains no nodes"))?;
    if nodes.iter().any(|node| {
        node.get("element")
            .and_then(Value::as_str)
            .is_some_and(|actual| accessibility_element_matches(actual, expected))
            && node.get("focused").and_then(Value::as_bool) == Some(true)
    }) {
        Ok(())
    } else {
        let focused: Vec<&str> = nodes
            .iter()
            .filter(|node| node.get("focused").and_then(Value::as_bool) == Some(true))
            .filter_map(|node| node.get("element").and_then(Value::as_str))
            .collect();
        bail!("element `{expected}` is not the rendered focused element; focused: {focused:?}")
    }
}

/// GPUI serializes ordinary element IDs as `Name("id")` in its native
/// accessibility bridge. Scenarios deliberately use the stable application
/// ID (`id`) so they do not depend on that transport representation.
fn accessibility_element_matches(actual: &str, expected: &str) -> bool {
    actual == expected
        || actual
            .strip_prefix("Name(\"")
            .and_then(|value| value.strip_suffix("\")"))
            == Some(expected)
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
struct PerceptualSnapshotThresholds {
    ssim_delta: f64,
    changed_pixel_ratio: f64,
}

impl Default for PerceptualSnapshotThresholds {
    fn default() -> Self {
        Self {
            ssim_delta: 0.01,
            changed_pixel_ratio: 0.003,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
struct PerceptualSnapshotDiff {
    ssim_delta: f64,
    changed_pixel_ratio: f64,
}

/// Compare a rendered screenshot with a perceptual baseline.
///
/// Syntax: `assert_perceptual_snapshot <name> <baseline.png>
/// [ssim=<0..1>] [changed=<0..1>]`. Set `SOTF_UPDATE_SNAPSHOTS=1` to
/// deliberately replace the baseline with the rendered screenshot.
pub(super) fn verb_assert_perceptual_snapshot(rest: &str, ctx: &Ctx) -> Result<()> {
    let mut parts = rest.split_whitespace();
    let name = parts.next().ok_or_else(|| {
        anyhow!(
            "assert_perceptual_snapshot needs `<name> <baseline.png> [ssim=<0..1>] [changed=<0..1>]`"
        )
    })?;
    let baseline = parts.next().ok_or_else(|| {
        anyhow!(
            "assert_perceptual_snapshot needs `<name> <baseline.png> [ssim=<0..1>] [changed=<0..1>]`"
        )
    })?;
    let thresholds = parse_perceptual_snapshot_thresholds(parts)?;
    let actual = qa_screenshot_path(ctx, name)?;
    let baseline = PathBuf::from(baseline);

    if snapshot_update_requested() {
        if let Some(parent) = baseline
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("creating snapshot baseline directory {}", parent.display())
            })?;
        }
        std::fs::copy(&actual, &baseline).with_context(|| {
            format!(
                "updating snapshot baseline {} from {}",
                baseline.display(),
                actual.display()
            )
        })?;
        if ctx.verbose {
            println!(" -> updated {}", baseline.display());
        }
        return Ok(());
    }

    let actual_image = image::open(&actual)
        .with_context(|| format!("opening actual screenshot {}", actual.display()))?
        .to_rgba8();
    let baseline_image = image::open(&baseline)
        .with_context(|| {
            format!(
                "opening perceptual baseline {} (set SOTF_UPDATE_SNAPSHOTS=1 to create it)",
                baseline.display()
            )
        })?
        .to_rgba8();
    let artifact_directory = qa_snapshot_artifact_directory(ctx)?;
    if actual_image.dimensions() != baseline_image.dimensions() {
        let artifacts = write_dimension_mismatch_artifacts(
            &actual_image,
            &baseline_image,
            &artifact_directory,
            name,
        )?;
        bail!(
            "perceptual snapshot `{name}` dimensions differ: actual {:?}, baseline {:?}; artifacts: {}",
            actual_image.dimensions(),
            baseline_image.dimensions(),
            artifacts.display()
        );
    }

    let difference = perceptual_snapshot_diff(&actual_image, &baseline_image);
    let metrics_path =
        write_perceptual_snapshot_metrics(&artifact_directory, name, difference, thresholds)?;
    if difference.ssim_delta > thresholds.ssim_delta
        || difference.changed_pixel_ratio > thresholds.changed_pixel_ratio
    {
        let artifacts =
            write_snapshot_artifacts(&actual_image, &baseline_image, &artifact_directory, name)?;
        bail!(
            "perceptual snapshot `{name}` exceeded budget: SSIM delta {:.6} > {:.6} or changed-pixel ratio {:.6} > {:.6}; actual: {}; baseline: {}; metrics: {}; artifacts: {}",
            difference.ssim_delta,
            thresholds.ssim_delta,
            difference.changed_pixel_ratio,
            thresholds.changed_pixel_ratio,
            actual.display(),
            baseline.display(),
            metrics_path.display(),
            artifacts.display()
        );
    }
    if ctx.verbose {
        println!(
            " -> ok (SSIM delta {:.6}, changed-pixel ratio {:.6}; metrics: {})",
            difference.ssim_delta,
            difference.changed_pixel_ratio,
            metrics_path.display()
        );
    }
    Ok(())
}

fn parse_perceptual_snapshot_thresholds<'a>(
    parts: impl Iterator<Item = &'a str>,
) -> Result<PerceptualSnapshotThresholds> {
    let mut thresholds = PerceptualSnapshotThresholds::default();
    let mut saw_ssim = false;
    let mut saw_changed = false;
    for part in parts {
        let (name, raw_value) = part.split_once('=').ok_or_else(|| {
            anyhow!("snapshot thresholds must use `ssim=<0..1>` or `changed=<0..1>`")
        })?;
        let value = raw_value
            .parse::<f64>()
            .with_context(|| format!("snapshot threshold `{name}` must be a number"))?;
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            bail!("snapshot threshold `{name}` must be between 0 and 1");
        }
        match name {
            "ssim" if !saw_ssim => {
                thresholds.ssim_delta = value;
                saw_ssim = true;
            }
            "changed" if !saw_changed => {
                thresholds.changed_pixel_ratio = value;
                saw_changed = true;
            }
            "ssim" | "changed" => bail!("duplicate snapshot threshold `{name}`"),
            _ => bail!("unknown snapshot threshold `{name}`"),
        }
    }
    Ok(thresholds)
}

fn snapshot_update_requested() -> bool {
    std::env::var("SOTF_UPDATE_SNAPSHOTS")
        .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "yes"))
}

fn write_perceptual_snapshot_metrics(
    directory: &Path,
    name: &str,
    difference: PerceptualSnapshotDiff,
    thresholds: PerceptualSnapshotThresholds,
) -> Result<PathBuf> {
    #[derive(serde::Serialize)]
    struct Artifact {
        schema_version: u32,
        source: &'static str,
        difference: PerceptualSnapshotDiff,
        thresholds: PerceptualSnapshotThresholds,
    }

    let path = directory.join(format!("{name}-metrics.json"));
    let bytes = serde_json::to_vec_pretty(&Artifact {
        schema_version: 1,
        source: "8x8-luminance-ssim+oklab-jnd",
        difference,
        thresholds,
    })
    .with_context(|| format!("serializing perceptual snapshot metrics for {name}"))?;
    std::fs::write(&path, bytes)
        .with_context(|| format!("writing perceptual snapshot metrics {}", path.display()))?;
    Ok(path)
}

fn perceptual_snapshot_diff(
    actual: &image::RgbaImage,
    baseline: &image::RgbaImage,
) -> PerceptualSnapshotDiff {
    debug_assert_eq!(actual.dimensions(), baseline.dimensions());
    let (width, height) = actual.dimensions();
    if width == 0 || height == 0 {
        return PerceptualSnapshotDiff {
            ssim_delta: 0.0,
            changed_pixel_ratio: 0.0,
        };
    }

    let mut changed_pixels = 0_u64;
    for (actual_pixel, baseline_pixel) in actual.pixels().zip(baseline.pixels()) {
        if perceptual_pixel_distance(actual_pixel.0, baseline_pixel.0) > 0.02 {
            changed_pixels += 1;
        }
    }

    let mut ssim_sum = 0.0;
    let mut block_count = 0_u64;
    for block_y in (0..height).step_by(8) {
        for block_x in (0..width).step_by(8) {
            let block_width = (width - block_x).min(8);
            let block_height = (height - block_y).min(8);
            let sample_count = f64::from(block_width * block_height);
            let mut sum_actual = 0.0;
            let mut sum_baseline = 0.0;
            let mut sum_actual_sq = 0.0;
            let mut sum_baseline_sq = 0.0;
            let mut sum_product = 0.0;
            for y in block_y..(block_y + block_height) {
                for x in block_x..(block_x + block_width) {
                    let actual_luma = pixel_luminance(actual.get_pixel(x, y).0);
                    let baseline_luma = pixel_luminance(baseline.get_pixel(x, y).0);
                    sum_actual += actual_luma;
                    sum_baseline += baseline_luma;
                    sum_actual_sq += actual_luma * actual_luma;
                    sum_baseline_sq += baseline_luma * baseline_luma;
                    sum_product += actual_luma * baseline_luma;
                }
            }
            let mean_actual = sum_actual / sample_count;
            let mean_baseline = sum_baseline / sample_count;
            let variance_actual =
                (sum_actual_sq / sample_count - mean_actual * mean_actual).max(0.0);
            let variance_baseline =
                (sum_baseline_sq / sample_count - mean_baseline * mean_baseline).max(0.0);
            let covariance = sum_product / sample_count - mean_actual * mean_baseline;
            let c1 = 0.01_f64.powi(2);
            let c2 = 0.03_f64.powi(2);
            let numerator = (2.0 * mean_actual * mean_baseline + c1) * (2.0 * covariance + c2);
            let denominator = (mean_actual * mean_actual + mean_baseline * mean_baseline + c1)
                * (variance_actual + variance_baseline + c2);
            ssim_sum += (numerator / denominator).clamp(-1.0, 1.0);
            block_count += 1;
        }
    }

    PerceptualSnapshotDiff {
        ssim_delta: (1.0 - ssim_sum / block_count as f64).clamp(0.0, 1.0),
        changed_pixel_ratio: changed_pixels as f64 / f64::from(width * height),
    }
}

fn pixel_luminance(pixel: [u8; 4]) -> f64 {
    let alpha = f64::from(pixel[3]) / 255.0;
    let red = linear_srgb(pixel[0]) * alpha;
    let green = linear_srgb(pixel[1]) * alpha;
    let blue = linear_srgb(pixel[2]) * alpha;
    0.2126 * red + 0.7152 * green + 0.0722 * blue
}

fn perceptual_pixel_distance(actual: [u8; 4], baseline: [u8; 4]) -> f64 {
    let actual_lab = oklab(actual);
    let baseline_lab = oklab(baseline);
    let alpha_delta = (f64::from(actual[3]) - f64::from(baseline[3])) / 255.0;
    ((actual_lab.0 - baseline_lab.0).powi(2)
        + (actual_lab.1 - baseline_lab.1).powi(2)
        + (actual_lab.2 - baseline_lab.2).powi(2)
        + alpha_delta.powi(2))
    .sqrt()
}

fn oklab(pixel: [u8; 4]) -> (f64, f64, f64) {
    let alpha = f64::from(pixel[3]) / 255.0;
    let red = linear_srgb(pixel[0]) * alpha;
    let green = linear_srgb(pixel[1]) * alpha;
    let blue = linear_srgb(pixel[2]) * alpha;
    let l = (0.412_221_470_8 * red + 0.536_332_536_3 * green + 0.051_445_992_9 * blue).cbrt();
    let m = (0.211_903_498_2 * red + 0.680_699_545_1 * green + 0.107_396_956_6 * blue).cbrt();
    let s = (0.088_302_461_9 * red + 0.281_718_837_6 * green + 0.629_978_700_5 * blue).cbrt();
    (
        0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    )
}

fn linear_srgb(channel: u8) -> f64 {
    let value = f64::from(channel) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn qa_screenshot_path(ctx: &Ctx, name: &str) -> Result<std::path::PathBuf> {
    if name.is_empty()
        || !name.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        bail!("screenshot name must contain only ASCII letters, digits, '-' or '_'");
    }
    let health = fetch_health(ctx)?;
    let qa_directory = health
        .get("value")
        .and_then(|value| value.get("qa_directory"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("health response does not contain qa_directory"))?;
    Ok(std::path::Path::new(qa_directory)
        .join("screenshots")
        .join(format!("{name}.png")))
}

fn qa_snapshot_artifact_directory(ctx: &Ctx) -> Result<PathBuf> {
    let health = fetch_health(ctx)?;
    let qa_directory = health
        .get("value")
        .and_then(|value| value.get("qa_directory"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("health response does not contain qa_directory"))?;
    let directory = PathBuf::from(qa_directory).join("snapshot-diffs");
    std::fs::create_dir_all(&directory).with_context(|| {
        format!(
            "creating snapshot artifact directory {}",
            directory.display()
        )
    })?;
    Ok(directory)
}

fn write_dimension_mismatch_artifacts(
    actual: &image::RgbaImage,
    baseline: &image::RgbaImage,
    directory: &Path,
    name: &str,
) -> Result<PathBuf> {
    actual
        .save(directory.join(format!("{name}-actual.png")))
        .with_context(|| format!("writing actual snapshot artifact for {name}"))?;
    baseline
        .save(directory.join(format!("{name}-expected.png")))
        .with_context(|| format!("writing expected snapshot artifact for {name}"))?;
    Ok(directory.to_path_buf())
}

pub(super) fn write_snapshot_artifacts(
    actual: &image::RgbaImage,
    baseline: &image::RgbaImage,
    directory: &Path,
    name: &str,
) -> Result<PathBuf> {
    debug_assert_eq!(actual.dimensions(), baseline.dimensions());
    let (width, height) = actual.dimensions();
    let mut difference = image::RgbaImage::new(width, height);
    let mut comparison = image::RgbaImage::new(width.saturating_mul(3), height);

    for y in 0..height {
        for x in 0..width {
            let actual_pixel = actual.get_pixel(x, y);
            let baseline_pixel = baseline.get_pixel(x, y);
            let difference_pixel = image::Rgba([
                actual_pixel[0]
                    .abs_diff(baseline_pixel[0])
                    .saturating_mul(4),
                actual_pixel[1]
                    .abs_diff(baseline_pixel[1])
                    .saturating_mul(4),
                actual_pixel[2]
                    .abs_diff(baseline_pixel[2])
                    .saturating_mul(4),
                255,
            ]);
            *difference.get_pixel_mut(x, y) = difference_pixel;
            *comparison.get_pixel_mut(x, y) = *baseline_pixel;
            *comparison.get_pixel_mut(width + x, y) = *actual_pixel;
            *comparison.get_pixel_mut(width.saturating_mul(2) + x, y) = difference_pixel;
        }
    }

    baseline
        .save(directory.join(format!("{name}-expected.png")))
        .with_context(|| format!("writing expected snapshot artifact for {name}"))?;
    actual
        .save(directory.join(format!("{name}-actual.png")))
        .with_context(|| format!("writing actual snapshot artifact for {name}"))?;
    difference
        .save(directory.join(format!("{name}-diff.png")))
        .with_context(|| format!("writing diff snapshot artifact for {name}"))?;
    comparison
        .save(directory.join(format!("{name}-comparison.png")))
        .with_context(|| format!("writing comparison snapshot artifact for {name}"))?;
    Ok(directory.to_path_buf())
}

pub(super) fn mean_pixel_delta(actual: &[u8], baseline: &[u8]) -> f64 {
    debug_assert_eq!(actual.len(), baseline.len());
    if actual.is_empty() {
        return 0.0;
    }
    actual
        .iter()
        .zip(baseline)
        .map(|(actual, baseline)| (*actual as f64 - *baseline as f64).abs())
        .sum::<f64>()
        / (actual.len() as f64 * 255.0)
}

pub(super) fn verb_wait_until(rest: &str, ctx: &Ctx) -> Result<()> {
    let cmp = parse_compare(rest)?;
    let timeout = cmp.timeout.unwrap_or(Duration::from_secs(1));
    let deadline = Instant::now() + timeout;
    let mut last = Value::Null;
    while Instant::now() < deadline {
        match verb_query(&cmp.path, ctx) {
            Ok(v) => {
                if cmp.matches(&v) {
                    if ctx.verbose {
                        println!("    -> matched ({v})");
                    }
                    return Ok(());
                }
                last = v;
            }
            Err(e) => last = Value::String(format!("{e}")),
        }
        sleep(Duration::from_millis(50));
    }
    bail!(
        "wait_until timed out after {:?}: {} {} {} (last seen: {})",
        timeout,
        cmp.path,
        cmp.op.as_str(),
        cmp.expected_text,
        last
    );
}

/// Wait until the rendered selector snapshot has remained unchanged for a
/// short quiet period. This avoids arbitrary sleeps after layout/input work.
pub(super) fn verb_wait_idle(rest: &str, ctx: &Ctx) -> Result<()> {
    let timeout = if rest.trim().is_empty() {
        Duration::from_secs(2)
    } else {
        super::parse::parse_duration(rest.trim())?
    };
    let quiet_period = Duration::from_millis(150);
    let deadline = Instant::now() + timeout;
    let mut snapshot = fetch_elements(ctx)?;
    let mut stable_since = Instant::now();

    while Instant::now() < deadline {
        sleep(Duration::from_millis(25));
        let next_snapshot = fetch_elements(ctx)?;
        if next_snapshot == snapshot {
            if stable_since.elapsed() >= quiet_period {
                return Ok(());
            }
        } else {
            snapshot = next_snapshot;
            stable_since = Instant::now();
        }
    }

    bail!(
        "wait_idle timed out after {timeout:?}; rendered selectors did not remain stable for {quiet_period:?}"
    )
}

pub(super) fn verb_key(rest: &str, ctx: &Ctx) -> Result<()> {
    let keystroke = rest.trim();
    if keystroke.is_empty() {
        bail!("key verb needs a keystroke");
    }
    let body = serde_json::json!({ "keystroke": keystroke });
    post_dev_json(ctx, "/key", &body, &format!("key `{keystroke}`"))?;
    Ok(())
}

/// Type text through the same key-dispatch path that a keyboard uses. The
/// server expands the bounded text payload into individual GPUI key events on
/// its UI thread; it is not a state mutation hook.
pub(super) fn verb_type(rest: &str, ctx: &Ctx) -> Result<()> {
    let text = parse_typed_text(rest)?;
    let body = json!({ "text": text });
    post_dev_json(ctx, "/text", &body, "type text")?;
    Ok(())
}

/// Decode a JSON string when a scenario needs spaces, `#`, or escapes; plain
/// text remains convenient for simple cases.
pub(super) fn parse_typed_text(rest: &str) -> Result<String> {
    if rest.is_empty() {
        bail!("type verb needs text");
    }
    if rest.starts_with('"') {
        return serde_json::from_str(rest).context("quoted type text must be a JSON string");
    }
    Ok(rest.to_owned())
}

pub(super) fn verb_click(rest: &str, ctx: &Ctx) -> Result<()> {
    let selector = rest.trim();
    if selector.is_empty() {
        bail!("click verb needs a selector");
    }
    let body = serde_json::json!({ "selector": selector });
    post_dev_json(ctx, "/click", &body, &format!("click `{selector}`"))?;
    Ok(())
}

/// Move the pointer over a tracked element without pressing a button.
pub(super) fn verb_hover(rest: &str, ctx: &Ctx) -> Result<()> {
    let selector = rest.trim();
    if selector.is_empty() {
        bail!("hover verb needs a selector");
    }
    let body = json!({ "selector": selector });
    post_dev_json(ctx, "/hover", &body, &format!("hover `{selector}`"))?;
    Ok(())
}

/// Drag from one tracked selector to another with a left-button gesture.
pub(super) fn verb_drag(rest: &str, ctx: &Ctx) -> Result<()> {
    let mut selectors = rest.split_whitespace();
    let source = selectors
        .next()
        .ok_or_else(|| anyhow!("drag verb needs a source selector"))?;
    let target = selectors
        .next()
        .ok_or_else(|| anyhow!("drag verb needs a target selector"))?;
    if selectors.next().is_some() {
        bail!("drag verb accepts exactly a source and target selector");
    }
    let body = json!({ "source": source, "target": target });
    post_dev_json(
        ctx,
        "/drag",
        &body,
        &format!("drag `{source}` to `{target}`"),
    )?;
    Ok(())
}

/// Scroll a tracked selector by a signed vertical pixel delta.
pub(super) fn verb_scroll(rest: &str, ctx: &Ctx) -> Result<()> {
    let (selector, delta_y) = split2(rest);
    if selector.is_empty() || delta_y.trim().is_empty() {
        bail!("scroll verb needs `<selector> <delta_y>`");
    }
    let delta_y: f32 = delta_y
        .trim()
        .parse()
        .context("scroll delta_y must be a number")?;
    if !delta_y.is_finite() {
        bail!("scroll delta_y must be finite");
    }
    let body = json!({ "selector": selector, "delta_y": delta_y });
    post_dev_json(
        ctx,
        "/scroll",
        &body,
        &format!("scroll `{selector}` by {delta_y}"),
    )?;
    Ok(())
}

/// Click a measured viewport point through native pointer events.
pub(super) fn verb_click_at(rest: &str, ctx: &Ctx) -> Result<()> {
    coordinate_click(rest, ctx, 1)
}

pub(super) fn verb_double_click_at(rest: &str, ctx: &Ctx) -> Result<()> {
    coordinate_click(rest, ctx, 2)
}

fn coordinate_click(rest: &str, ctx: &Ctx, click_count: usize) -> Result<()> {
    let values = rest
        .split_whitespace()
        .map(str::parse::<f64>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("click_at requires numeric x and y")?;
    if values.len() != 2 || values.iter().any(|value| !value.is_finite()) {
        bail!("click_at needs finite `<x> <y>`");
    }
    let response = ctx.client.get(format!("{}/snapshot", ctx.base)).send()?;
    let snapshot = parse_dev_response(response, "snapshot for coordinate click")?;
    let snapshot = snapshot.get("value").unwrap_or(&snapshot);
    let revision = snapshot
        .get("state_revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("snapshot contains no state revision"))?;
    for phase in ["move", "down", "up"] {
        post_dev_json(
            ctx,
            "/input",
            &json!({
                "kind": "pointer", "phase": phase, "x": values[0], "y": values[1],
                "button": 0, "click_count": click_count, "viewport_revision": revision,
            }),
            "coordinate click",
        )?;
    }
    Ok(())
}

/// Send wheel input at an explicit viewport point, using a fresh snapshot revision.
pub(super) fn verb_scroll_at(rest: &str, ctx: &Ctx) -> Result<()> {
    let values = rest
        .split_whitespace()
        .map(str::parse::<f64>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("scroll_at requires numeric x, y, and delta_y")?;
    if values.len() != 3 || values.iter().any(|value| !value.is_finite()) {
        bail!("scroll_at needs finite `<x> <y> <delta_y>`");
    }
    let response = ctx.client.get(format!("{}/snapshot", ctx.base)).send()?;
    let snapshot = parse_dev_response(response, "snapshot for coordinate scroll")?;
    let snapshot = snapshot.get("value").unwrap_or(&snapshot);
    let revision = snapshot
        .get("state_revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("snapshot contains no state revision"))?;
    post_dev_json(
        ctx,
        "/input",
        &json!({
            "kind": "scroll", "x": values[0], "y": values[1],
            "delta_x": 0.0, "delta_y": values[2], "viewport_revision": revision,
        }),
        "coordinate scroll",
    )?;
    Ok(())
}

/// Bring an already-rendered target's click point inside a vertical scrollport.
pub(super) fn verb_scroll_into_view(rest: &str, ctx: &Ctx) -> Result<()> {
    let (container, target) = split2(rest);
    let target = target.trim();
    if container.is_empty() || target.is_empty() {
        bail!("scroll_into_view needs `<container> <target>`");
    }
    for _ in 0..4 {
        let elements = fetch_elements(ctx)?;
        let port = find_element(&elements, container)
            .ok_or_else(|| anyhow!("scroll container `{container}` is not rendered"))?;
        let item = find_element(&elements, target).ok_or_else(|| {
            anyhow!("target `{target}` is not rendered; scroll its region into view first")
        })?;
        let health = fetch_health(ctx)?;
        let viewport = health
            .get("value")
            .and_then(|value| value.get("viewport"))
            .ok_or_else(|| anyhow!("health response does not contain viewport"))?;
        if !element_is_within_viewport(port, viewport) {
            bail!("scroll container `{container}` is clipped by the window");
        }
        let center = |element: &Value| -> Result<f64> {
            let y = element.get("y").and_then(Value::as_f64);
            let height = element.get("h").and_then(Value::as_f64);
            match (y, height) {
                (Some(y), Some(height)) if y.is_finite() && height.is_finite() && height > 0.0 => {
                    Ok(y + height / 2.0)
                }
                _ => bail!("invalid scroll geometry: {element}"),
            }
        };
        let delta = center(port)? - center(item)?;
        let height = port.get("h").and_then(Value::as_f64).unwrap_or(0.0);
        if delta.abs() < (height / 2.0 - 1.0).max(0.0) {
            return Ok(());
        }
        post_dev_json(
            ctx,
            "/scroll",
            &json!({"selector": container, "delta_y": delta}),
            "center scroll target",
        )?;
        sleep(Duration::from_millis(100));
    }
    let elements = fetch_elements(ctx)?;
    let port = find_element(&elements, container);
    let item = find_element(&elements, target);
    bail!("could not bring `{target}` inside `{container}`; container={port:?}, target={item:?}")
}

/// Resize the window content area to a deterministic viewport.
pub(super) fn verb_resize(rest: &str, ctx: &Ctx) -> Result<()> {
    let (width, height) = parse_resize_dimensions(rest)?;
    let body = json!({ "width": width, "height": height });
    post_dev_json(ctx, "/resize", &body, &format!("resize {width}x{height}"))?;
    Ok(())
}

/// Capture the current frame as `<qa-dir>/screenshots/<name>.png`.
pub(super) fn verb_screenshot(rest: &str, ctx: &Ctx) -> Result<()> {
    let name = rest.trim();
    if name.is_empty() {
        bail!("screenshot verb needs a name");
    }
    if !name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
    {
        bail!("screenshot name must contain only ASCII letters, digits, '-' or '_'");
    }
    let body = json!({ "name": name });
    post_dev_json(ctx, "/screenshot", &body, &format!("screenshot `{name}`"))?;
    Ok(())
}

pub(super) fn parse_resize_dimensions(rest: &str) -> Result<(f32, f32)> {
    let mut dimensions = rest.split_whitespace();
    let width: f32 = dimensions
        .next()
        .ok_or_else(|| anyhow!("resize verb needs `<width> <height>`"))?
        .parse()
        .context("resize width must be a number")?;
    let height: f32 = dimensions
        .next()
        .ok_or_else(|| anyhow!("resize verb needs `<width> <height>`"))?
        .parse()
        .context("resize height must be a number")?;
    if dimensions.next().is_some() {
        bail!("resize verb accepts exactly width and height");
    }
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        bail!("resize width and height must be finite positive numbers");
    }
    Ok((width, height))
}

pub(super) fn verb_elements(ctx: &Ctx) -> Result<()> {
    let json = fetch_elements(ctx)?;
    let list = json
        .get("elements")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if list.is_empty() {
        println!("    (no tracked elements yet)");
    } else {
        for el in list {
            let sel = el.get("selector").and_then(Value::as_str).unwrap_or("?");
            let cx = el.get("cx").and_then(Value::as_f64).unwrap_or(0.0);
            let cy = el.get("cy").and_then(Value::as_f64).unwrap_or(0.0);
            println!("    {sel:<40} @ ({cx:.0}, {cy:.0})");
        }
    }
    Ok(())
}

/// Print the rendered platform accessibility tree for inspection/debugging.
pub(super) fn verb_accessibility(ctx: &Ctx) -> Result<()> {
    let response = ctx
        .client
        .get(format!("{}/accessibility", ctx.base))
        .send()?;
    let json = parse_dev_response(response, "accessibility")?;
    let value = json
        .get("value")
        .ok_or_else(|| anyhow!("accessibility response contains no value"))?;
    println!(
        "    {} accessibility nodes ({} focusable)",
        value.get("node_count").and_then(Value::as_u64).unwrap_or(0),
        value
            .get("focusable_node_count")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    );
    let unnamed_focusables = value
        .get("nodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|node| node.get("focusable") == Some(&Value::Bool(true)))
        .filter_map(|node| {
            let label = node
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or_default();
            label.trim().is_empty().then(|| {
                node.get("element")
                    .and_then(Value::as_str)
                    .unwrap_or("<unknown>")
                    .to_string()
            })
        })
        .collect::<Vec<_>>();
    if !unnamed_focusables.is_empty() {
        bail!(
            "accessibility tree contains unnamed focusable elements: {}",
            unnamed_focusables.join(", ")
        );
    }
    if ctx.verbose {
        for node in value
            .get("nodes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            println!("    {node}");
        }
    }
    Ok(())
}

/// Assert that a stable rendered selector exists and has visible area.
pub(super) fn verb_assert_visible(rest: &str, ctx: &Ctx) -> Result<()> {
    let selector = rest.trim();
    if selector.is_empty() {
        bail!("assert_visible verb needs a selector");
    }

    let elements = fetch_elements(ctx)?;
    let element = find_element(&elements, selector)
        .ok_or_else(|| anyhow!("rendered selector `{selector}` is not present"))?;
    if !element_is_visible(element) {
        bail!("rendered selector `{selector}` has empty or invalid bounds: {element}");
    }
    Ok(())
}

/// Assert that a selector from a previous view is no longer rendered.
pub(super) fn verb_assert_absent(rest: &str, ctx: &Ctx) -> Result<()> {
    let selector = rest.trim();
    if selector.is_empty() {
        bail!("assert_absent verb needs a selector");
    }
    if find_element(&fetch_elements(ctx)?, selector).is_some() {
        bail!("rendered selector `{selector}` is still present");
    }
    Ok(())
}

/// Assert that a rendered selector fits entirely inside the current viewport.
pub(super) fn verb_assert_in_viewport(rest: &str, ctx: &Ctx) -> Result<()> {
    let selector = rest.trim();
    if selector.is_empty() {
        bail!("assert_in_viewport verb needs a selector");
    }

    let elements = fetch_elements(ctx)?;
    let element = find_element(&elements, selector)
        .ok_or_else(|| anyhow!("rendered selector `{selector}` is not present"))?;
    let health = fetch_health(ctx)?;
    let viewport = health
        .get("value")
        .and_then(|value| value.get("viewport"))
        .ok_or_else(|| anyhow!("health response does not contain a viewport"))?;
    if !element_is_within_viewport(element, viewport) {
        bail!(
            "rendered selector `{selector}` is outside the viewport: element={element}, viewport={viewport}"
        );
    }
    Ok(())
}

/// Assert that two rendered selectors do not overlap.
pub(super) fn verb_assert_non_overlapping(rest: &str, ctx: &Ctx) -> Result<()> {
    let mut selectors = rest.split_whitespace();
    let first = selectors
        .next()
        .ok_or_else(|| anyhow!("assert_non_overlapping needs two selectors"))?;
    let second = selectors
        .next()
        .ok_or_else(|| anyhow!("assert_non_overlapping needs two selectors"))?;
    if selectors.next().is_some() {
        bail!("assert_non_overlapping accepts exactly two selectors");
    }

    let elements = fetch_elements(ctx)?;
    let first_element = find_element(&elements, first)
        .ok_or_else(|| anyhow!("rendered selector `{first}` is not present"))?;
    let second_element = find_element(&elements, second)
        .ok_or_else(|| anyhow!("rendered selector `{second}` is not present"))?;
    if elements_overlap(first_element, second_element) {
        bail!("rendered selectors `{first}` and `{second}` overlap");
    }
    Ok(())
}

/// Assert an explicit semantic state published by the rendered selector.
/// `assert_enabled transport.play == true`, `assert_selected ...`, and
/// `assert_expanded ...` deliberately fail when the control has not supplied
/// that state, rather than falling back to unrelated application-model data.
pub(super) fn verb_assert_element_state(rest: &str, field: &str, ctx: &Ctx) -> Result<()> {
    let (selector, comparison) = split2(rest);
    if selector.is_empty() || comparison.trim().is_empty() {
        bail!("assert_{field} needs `<selector> <operator> <literal>`");
    }
    let comparison = parse_compare(&format!("state {comparison}"))?;
    let elements = fetch_elements(ctx)?;
    let element = find_element(&elements, selector)
        .ok_or_else(|| anyhow!("rendered selector `{selector}` is not present"))?;
    let actual = element_state_value(element, selector, field)?;
    if !comparison.matches(actual) {
        bail!(
            "semantic assertion failed: `{selector}` {field} {} {} (got {actual})",
            comparison.op.as_str(),
            comparison.expected_text,
        );
    }
    if ctx.verbose {
        println!("    -> ok ({field}={actual})");
    }
    Ok(())
}

fn fetch_elements(ctx: &Ctx) -> Result<Value> {
    let resp = ctx.client.get(format!("{}/elements", ctx.base)).send()?;
    let mut elements = parse_dev_response(resp, "elements")?;
    // Protocol-v2 attaches command sequence and timing metadata to every JSON
    // reply. Element snapshots are compared for rendered stability, so that
    // transport metadata must not make an otherwise idle UI look busy.
    if let Some(object) = elements.as_object_mut() {
        object.remove("meta");
    }
    Ok(elements)
}

fn fetch_health(ctx: &Ctx) -> Result<Value> {
    let resp = ctx.client.get(format!("{}/health", ctx.base)).send()?;
    parse_dev_response(resp, "health")
}

fn find_element<'a>(elements: &'a Value, selector: &str) -> Option<&'a Value> {
    elements
        .get("elements")?
        .as_array()?
        .iter()
        .find(|element| element.get("selector").and_then(Value::as_str) == Some(selector))
}

fn element_state_value<'a>(element: &'a Value, selector: &str, field: &str) -> Result<&'a Value> {
    element.get(field).ok_or_else(|| {
        anyhow!(
            "rendered selector `{selector}` does not publish `{field}` state; add dev_track_with_state at its painted control"
        )
    })
}

fn element_is_visible(element: &Value) -> bool {
    let width = element.get("w").and_then(Value::as_f64);
    let height = element.get("h").and_then(Value::as_f64);
    matches!((width, height), (Some(width), Some(height)) if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0)
}

fn element_is_within_viewport(element: &Value, viewport: &Value) -> bool {
    let x = element.get("x").and_then(Value::as_f64);
    let y = element.get("y").and_then(Value::as_f64);
    let width = element.get("w").and_then(Value::as_f64);
    let height = element.get("h").and_then(Value::as_f64);
    let viewport_width = viewport.get("width").and_then(Value::as_f64);
    let viewport_height = viewport.get("height").and_then(Value::as_f64);

    matches!(
        (x, y, width, height, viewport_width, viewport_height),
        (Some(x), Some(y), Some(width), Some(height), Some(viewport_width), Some(viewport_height))
            if x.is_finite()
                && y.is_finite()
                && width.is_finite()
                && height.is_finite()
                && viewport_width.is_finite()
                && viewport_height.is_finite()
                && x >= 0.0
                && y >= 0.0
                && width > 0.0
                && height > 0.0
                && x + width <= viewport_width
                && y + height <= viewport_height
    )
}

fn elements_overlap(first: &Value, second: &Value) -> bool {
    let rect = |element: &Value| {
        Some((
            element.get("x")?.as_f64()?,
            element.get("y")?.as_f64()?,
            element.get("w")?.as_f64()?,
            element.get("h")?.as_f64()?,
        ))
    };
    let (
        Some((first_x, first_y, first_w, first_h)),
        Some((second_x, second_y, second_w, second_h)),
    ) = (rect(first), rect(second))
    else {
        // Missing/invalid bounds are caught by assert_visible; do not claim a
        // non-overlap result from malformed geometry.
        return true;
    };

    first_x < second_x + second_w
        && first_x + first_w > second_x
        && first_y < second_y + second_h
        && first_y + first_h > second_y
}

#[cfg(test)]
mod rendered_selector_tests {
    use super::{
        accessibility_element_matches, element_is_visible, element_is_within_viewport,
        element_state_value, elements_overlap, find_element,
    };
    use crate::parse::parse_compare;
    use serde_json::json;

    #[test]
    fn accessibility_focus_matches_stable_application_id() {
        assert!(accessibility_element_matches(
            "Name(\"playlist-name\")",
            "playlist-name"
        ));
        assert!(accessibility_element_matches(
            "playlist-name",
            "playlist-name"
        ));
        assert!(!accessibility_element_matches(
            "Name(\"other\")",
            "playlist-name"
        ));
    }

    #[test]
    fn rendered_selector_requires_positive_bounds() {
        let elements = json!({
            "elements": [
                { "selector": "visible", "w": 12.0, "h": 8.0 },
                { "selector": "empty", "w": 0.0, "h": 8.0 },
            ]
        });

        assert!(element_is_visible(
            find_element(&elements, "visible").expect("visible selector")
        ));
        assert!(!element_is_visible(
            find_element(&elements, "empty").expect("empty selector")
        ));
        assert!(find_element(&elements, "missing").is_none());
    }

    #[test]
    fn rendered_selector_must_fit_viewport() {
        let viewport = json!({ "width": 100.0, "height": 60.0 });
        assert!(element_is_within_viewport(
            &json!({ "x": 1.0, "y": 2.0, "w": 90.0, "h": 50.0 }),
            &viewport
        ));
        assert!(!element_is_within_viewport(
            &json!({ "x": 10.0, "y": 2.0, "w": 100.0, "h": 50.0 }),
            &viewport
        ));
    }

    #[test]
    fn rendered_selector_overlap_uses_bounds_intersection() {
        let first = json!({ "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0 });
        let touching = json!({ "x": 10.0, "y": 0.0, "w": 5.0, "h": 5.0 });
        let overlapping = json!({ "x": 9.0, "y": 0.0, "w": 5.0, "h": 5.0 });

        assert!(!elements_overlap(&first, &touching));
        assert!(elements_overlap(&first, &overlapping));
    }

    #[test]
    fn rendered_selector_state_is_explicit_and_comparable() {
        let element = json!({ "selector": "transport.play", "enabled": true, "selected": false });
        let enabled = element_state_value(&element, "transport.play", "enabled").unwrap();
        assert!(parse_compare("state == true").unwrap().matches(enabled));
        assert!(element_state_value(&element, "transport.play", "expanded").is_err());
    }
}

pub(super) fn verb_export_room_eq_json(rest: &str, ctx: &Ctx) -> Result<()> {
    let path = rest.trim();
    let body = if path.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::json!({ "path": path })
    };
    let json = post_dev_json(ctx, "/qa/room-eq/export-json", &body, "RoomEQ JSON export")?;
    if ctx.verbose {
        let value = json.get("value").cloned().unwrap_or(Value::Null);
        println!("    -> {value}");
    }
    Ok(())
}

pub(super) fn verb_focus(rest: &str, ctx: &Ctx) -> Result<()> {
    let target = rest.trim();
    if target.is_empty() {
        bail!("focus verb needs a screen name");
    }
    let action_name = focus_action_name(target)?;
    verb_action(&action_name, ctx)
}

pub(super) fn verb_plugin_add(rest: &str, ctx: &Ctx) -> Result<()> {
    let plugin_type = rest.trim();
    if plugin_type.is_empty() {
        bail!("plugin_add needs a plugin type");
    }
    let body = json!({ "name": "PluginAdd", "payload": { "plugin_type": plugin_type } });
    post_dev_json(ctx, "/action", &body, "plugin_add")?;
    Ok(())
}

pub(super) fn verb_plugin_remove(rest: &str, ctx: &Ctx) -> Result<()> {
    let index: usize = rest
        .trim()
        .parse()
        .context("plugin_remove needs an index")?;
    let body = json!({ "name": "PluginRemove", "payload": { "index": index } });
    post_dev_json(ctx, "/action", &body, "plugin_remove")?;
    Ok(())
}

pub(super) fn verb_plugin_clear(_rest: &str, ctx: &Ctx) -> Result<()> {
    let body = json!({ "name": "PluginClear", "payload": {} });
    post_dev_json(ctx, "/action", &body, "plugin_clear")?;
    Ok(())
}

pub(super) fn verb_plugin_count(_rest: &str, ctx: &Ctx) -> Result<Value> {
    verb_query("plugins.count", ctx)
}

pub(super) fn verb_plugin_param_count(rest: &str, ctx: &Ctx) -> Result<Value> {
    let index: usize = rest
        .trim()
        .parse()
        .context("plugin_param_count needs an index")?;
    verb_query(&format!("plugins.plugin.{index}.param_count"), ctx)
}

pub(super) fn verb_plugin_param_set(rest: &str, ctx: &Ctx) -> Result<()> {
    let mut parts = rest.split_whitespace();
    let index: usize = parts
        .next()
        .ok_or_else(|| anyhow!("plugin_param_set needs index"))?
        .parse()?;
    let param_index: usize = parts
        .next()
        .ok_or_else(|| anyhow!("plugin_param_set needs param_index"))?
        .parse()?;
    let value: f64 = parts
        .next()
        .ok_or_else(|| anyhow!("plugin_param_set needs value"))?
        .parse()?;
    let body = json!({ "name": "PluginSetParam", "payload": { "index": index, "param_index": param_index, "value": value } });
    post_dev_json(ctx, "/action", &body, "plugin_param_set")?;
    Ok(())
}

pub(super) fn verb_plugin_param_get(rest: &str, ctx: &Ctx) -> Result<Value> {
    let mut parts = rest.split_whitespace();
    let index: usize = parts
        .next()
        .ok_or_else(|| anyhow!("plugin_param_get needs index"))?
        .parse()?;
    let param_index: usize = parts
        .next()
        .ok_or_else(|| anyhow!("plugin_param_get needs param_index"))?
        .parse()?;
    verb_query(
        &format!("plugins.plugin.{index}.param.{param_index}.value"),
        ctx,
    )
}

pub(super) fn verb_plugin_chain_save(rest: &str, ctx: &Ctx) -> Result<()> {
    let path = rest.trim();
    if path.is_empty() {
        bail!("plugin_chain_save needs a path");
    }
    let body = json!({ "name": "PluginChainSave", "payload": { "path": path } });
    post_dev_json(ctx, "/action", &body, "plugin_chain_save")?;
    Ok(())
}

pub(super) fn verb_plugin_chain_load(rest: &str, ctx: &Ctx) -> Result<()> {
    let path = rest.trim();
    if path.is_empty() {
        bail!("plugin_chain_load needs a path");
    }
    let body = json!({ "name": "PluginChainLoad", "payload": { "path": path } });
    post_dev_json(ctx, "/action", &body, "plugin_chain_load")?;
    Ok(())
}

#[cfg(test)]
mod perceptual_snapshot_tests {
    use image::{Rgba, RgbaImage};

    use super::{
        PerceptualSnapshotThresholds, parse_perceptual_snapshot_thresholds,
        perceptual_snapshot_diff,
    };

    #[test]
    fn identical_images_have_zero_perceptual_difference() {
        let image = RgbaImage::from_pixel(16, 16, Rgba([40, 80, 120, 255]));
        let difference = perceptual_snapshot_diff(&image, &image);
        assert_eq!(difference.ssim_delta, 0.0);
        assert_eq!(difference.changed_pixel_ratio, 0.0);
    }

    #[test]
    fn localized_visible_change_is_counted() {
        let baseline = RgbaImage::from_pixel(16, 16, Rgba([20, 20, 20, 255]));
        let mut actual = baseline.clone();
        for y in 0..8 {
            for x in 0..8 {
                actual.put_pixel(x, y, Rgba([240, 240, 240, 255]));
            }
        }
        let difference = perceptual_snapshot_diff(&actual, &baseline);
        assert!(difference.ssim_delta > 0.1);
        assert_eq!(difference.changed_pixel_ratio, 0.25);
    }

    #[test]
    fn threshold_parser_accepts_named_values_in_any_order() {
        let thresholds =
            parse_perceptual_snapshot_thresholds(["changed=0.02", "ssim=0.03"].into_iter())
                .unwrap();
        assert_eq!(
            thresholds,
            PerceptualSnapshotThresholds {
                ssim_delta: 0.03,
                changed_pixel_ratio: 0.02,
            }
        );
    }

    #[test]
    fn threshold_parser_rejects_unknown_duplicate_and_out_of_range_values() {
        assert!(parse_perceptual_snapshot_thresholds(["pixel=0.1"].into_iter()).is_err());
        assert!(
            parse_perceptual_snapshot_thresholds(["ssim=0.1", "ssim=0.2"].into_iter()).is_err()
        );
        assert!(parse_perceptual_snapshot_thresholds(["changed=1.1"].into_iter()).is_err());
    }
}
