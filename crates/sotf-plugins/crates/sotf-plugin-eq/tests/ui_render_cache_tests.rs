use std::path::Path;

fn crate_source(relative: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|err| panic!("failed to read {relative}: {err}"))
}

fn workspace_source(relative: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../..")
            .join(relative),
    )
    .unwrap_or_else(|err| panic!("failed to read {relative}: {err}"))
}

#[test]
fn eq_ui_uses_shared_plugins_gpui_curve_cache() {
    let render = crate_source("src/ui/render.rs");

    assert!(render.contains("eq_frequency_points"));
    assert!(render.contains("eq_curve_cache"));
    assert!(render.contains("get_or_build"));
    assert!(render.contains("eq_curve_signature(filters, freq_points.len())"));
    assert!(
        !render.contains("let freq_points: Vec<f64>"),
        "EQ UI must not rebuild the frequency grid during render"
    );
    assert!(
        !render.contains("let band_response: Vec<f64>"),
        "EQ UI must not rebuild band response vectors during render"
    );
}

#[test]
fn plugins_gpui_owns_eq_curve_cache_and_static_grid() {
    let lib = workspace_source("crates/sotf-plugins/crates/plugins-gpui/src/lib.rs");
    let cache = workspace_source("crates/sotf-plugins/crates/plugins-gpui/src/eq_curve_cache.rs");

    assert!(lib.contains("pub mod eq_curve_cache;"));
    assert!(lib.contains("EqCurveRenderCache"));
    assert!(cache.contains("static EQ_FREQUENCY_POINTS"));
    assert!(cache.contains("OnceLock<Vec<f64>>"));
    assert!(cache.contains("EqCurveRenderCache"));
    assert!(
        !cache.contains("px("),
        "cache module must stay data-only and not introduce raw UI pixel tokens"
    );
}
