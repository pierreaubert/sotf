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

pub(super) fn typed_keystroke(character: char) -> String {
    match character {
        ' ' => "space".to_owned(),
        '\t' => "tab".to_owned(),
        '\n' => "enter".to_owned(),
        _ => character.to_string(),
    }
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
