use crate::pages::level_meter::LevelMeterPage;
use crate::runner::{E2ERunner, TestScenario};
use gpui::{TestAppContext, VisualTestContext, WindowHandle};
use sotf_audio_player_gpui::ui::PlayerView;
use std::error::Error;

pub struct MuteSoloLevelMeterScenario;

impl TestScenario for MuteSoloLevelMeterScenario {
    fn name(&self) -> &'static str {
        "Mute/Solo Level Meter"
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        use crate::driver::AppDriver;
        let mut driver = AppDriver::new(cx, window);

        // No playback needed: App::new() already initializes default stereo meter groups.
        let mut meter_page = LevelMeterPage::new(&mut driver);
        meter_page.ensure_visible();

        // Stereo layout has 1 group "L/R" containing channels 0 (L) and 1 (R).
        println!("Checking initial state...");
        let group_count = meter_page
            .driver
            .read_app(|app| app.level_meters.groups.len());
        assert!(
            group_count >= 1,
            "Should have at least 1 meter group (L/R). Got {}",
            group_count
        );
        let channel_count = meter_page.driver.read_app(|app| {
            app.level_meters
                .groups
                .first()
                .map(|g| g.channels.len())
                .unwrap_or(0)
        });
        assert!(
            channel_count >= 2,
            "L/R group should have at least 2 channels. Got {}",
            channel_count
        );
        assert!(
            !meter_page.is_muted(0),
            "Group 0 should not be muted initially"
        );

        // 1. Test Mute on L/R group — mutes both L and R in matrix
        println!("Toggling mute on Group 0 (L/R)...");
        meter_page.toggle_mute(0);

        println!("Verifying mute state...");
        assert!(
            meter_page.is_muted(0),
            "Group 0 (L/R) should be muted in UI"
        );
        assert!(
            meter_page.get_matrix_channel_mute_state(0),
            "Channel 0 (L) should be muted in Matrix"
        );
        assert!(
            meter_page.get_matrix_channel_mute_state(1),
            "Channel 1 (R) should be muted in Matrix (same group)"
        );

        println!("Untoggling mute...");
        meter_page.toggle_mute(0);
        assert!(!meter_page.is_muted(0), "Group 0 should be unmuted");
        assert!(
            !meter_page.get_matrix_channel_mute_state(0),
            "Channel 0 should be unmuted in Matrix"
        );
        assert!(
            !meter_page.get_matrix_channel_mute_state(1),
            "Channel 1 should be unmuted in Matrix"
        );

        // 2. Test Solo
        println!("Toggling solo on Group 0 (L/R)...");
        meter_page.toggle_solo(0);

        println!("Verifying solo state...");
        assert!(meter_page.is_soloed(0), "Group 0 should be soloed in UI");
        assert!(
            meter_page.get_matrix_channel_solo_state(0),
            "Channel 0 (L) should be soloed in Matrix"
        );
        assert!(
            meter_page.get_matrix_channel_solo_state(1),
            "Channel 1 (R) should be soloed in Matrix (same group)"
        );

        // Unsolo
        meter_page.toggle_solo(0);
        assert!(!meter_page.is_soloed(0), "Group 0 should be unsoloed");

        // 3. Test Dim
        println!("Toggling dim on Group 0 (L/R)...");
        meter_page.toggle_dim(0);

        println!("Verifying dim state...");
        assert!(meter_page.is_dimmed(0), "Group 0 should be dimmed in UI");

        println!("Success: Matrix plugin states correctly reflect UI mute/solo/dim actions.");
        Ok(())
    }
}

#[gpui::test]
async fn test_mute_solo_level_meter(cx: &mut TestAppContext) {
    let runner = E2ERunner::new(MuteSoloLevelMeterScenario);
    runner.run(cx).await.unwrap();
}

#[cfg(feature = "dev-api")]
#[test]
fn loudness_control_target_follows_displayed_analyzer_identity() {
    use sotf_audio_player::PluginGraph;
    use sotf_audio_player_gpui::components::plugins::custom_view_registry::loudness_control_engine_index;

    let graph = PluginGraph::with_default_rack();
    let input = graph.input_monitor_engine_index();
    let output = graph.output_monitor_engine_index();
    assert!(input.is_some() && output.is_some() && input != output);
    assert_eq!(
        loudness_control_engine_index(&graph, Some(101), Some(101), Some(202)),
        input
    );
    assert_eq!(
        loudness_control_engine_index(&graph, Some(202), Some(101), Some(202)),
        output
    );
    assert_eq!(
        loudness_control_engine_index(&graph, Some(303), Some(101), Some(202)),
        None
    );
    assert_eq!(
        loudness_control_engine_index(&graph, Some(0), Some(0), Some(202)),
        None
    );
}

#[cfg(feature = "dev-api")]
struct ProgrammeMaximumTruePeakView {
    loudness: sotf_audio_player::LoudnessData,
    language: sotf_audio_player_gpui::i18n::Language,
}

#[cfg(feature = "dev-api")]
fn try_tracked_meter_element(
    window_id: u64,
    selector: &str,
) -> Option<sotf_audio_player_gpui::app::dev_api::registry::TrackedElement> {
    let snapshot = sotf_audio_player_gpui::app::dev_api::registry::snapshot_for(window_id);
    snapshot
        .iter()
        .into_iter()
        .find(|(tracked, _)| tracked == selector)
        .map(|(_, element)| element.clone())
}

#[cfg(feature = "dev-api")]
fn tracked_meter_element(
    window_id: u64,
    selector: &str,
) -> sotf_audio_player_gpui::app::dev_api::registry::TrackedElement {
    let snapshot = sotf_audio_player_gpui::app::dev_api::registry::snapshot_for(window_id);
    snapshot
        .iter()
        .into_iter()
        .find(|(tracked, _)| tracked == selector)
        .map(|(_, element)| element.clone())
        .unwrap_or_else(|| {
            let selectors = snapshot
                .iter()
                .map(|(tracked, _)| tracked.as_str())
                .collect::<Vec<_>>();
            panic!("rendered selector {selector} should be published; got {selectors:?}")
        })
}

#[cfg(feature = "dev-api")]
fn assert_maximum_true_peak_layout(
    window_id: u64,
    panel_selector: &str,
    viewport_width: f32,
    viewport_height: f32,
) {
    use gpui::{Bounds, Pixels};

    let panel = tracked_meter_element(window_id, panel_selector).bounds;
    let summary = tracked_meter_element(window_id, "meters.lufs.maximum-true-peak-summary").bounds;
    let label = tracked_meter_element(window_id, "meters.lufs.maximum-true-peak-label").bounds;
    let value = tracked_meter_element(window_id, "meters.lufs.maximum-true-peak-value").bounds;
    let bar = tracked_meter_element(window_id, "meters.lufs.true-peak-bar.0").bounds;

    let right = |bounds: Bounds<Pixels>| f32::from(bounds.origin.x) + f32::from(bounds.size.width);
    let bottom =
        |bounds: Bounds<Pixels>| f32::from(bounds.origin.y) + f32::from(bounds.size.height);

    assert!(panel.origin.x >= gpui::px(0.0));
    assert!(panel.origin.y >= gpui::px(0.0));
    assert!(
        right(panel) <= viewport_width && bottom(panel) <= viewport_height,
        "the meters panel should fit in the compact viewport: {panel:?}"
    );
    assert!(summary.origin.x >= panel.origin.x);
    assert!(summary.origin.y >= panel.origin.y);
    assert!(
        right(summary) <= right(panel),
        "programme maximum summary is clipped horizontally by the meters panel: {summary:?}, panel {panel:?}"
    );
    assert!(
        bottom(summary) <= bottom(panel),
        "programme maximum summary is clipped vertically by the meters panel: {summary:?}, panel {panel:?}"
    );
    assert!(
        label.origin.y >= summary.origin.y && bottom(label) <= bottom(summary),
        "the Max TP label should fit inside its summary row: {label:?}, summary {summary:?}"
    );
    assert!(
        value.origin.y >= summary.origin.y && bottom(value) <= bottom(summary),
        "the Max TP value should fit inside its summary row: {value:?}, summary {summary:?}"
    );
    assert!(
        bottom(label) <= f32::from(value.origin.y),
        "the stacked Max TP label and value should not overlap: {label:?}, {value:?}"
    );
    assert!(
        bottom(summary) <= f32::from(bar.origin.y),
        "the programme maximum summary overlaps the first interval bar: {summary:?}, bar {bar:?}"
    );
    assert!(
        bar.origin.x >= panel.origin.x && right(bar) <= right(panel),
        "the first interval bar is clipped horizontally by the meters panel: {bar:?}, panel {panel:?}"
    );
}

#[cfg(feature = "dev-api")]
impl gpui::Render for ProgrammeMaximumTruePeakView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use gpui::InteractiveElement;
        use gpui::ParentElement;
        use gpui::Styled;
        use sotf_audio_player_gpui::app::dev_api::DevTrackExt;
        use sotf_audio_player_gpui::components::design::Ds;
        use sotf_audio_player_gpui::components::plugins::level_meters::render_lufs_with_true_peak;

        let theme = sotf_audio_player_gpui::theme::Theme::from_id(
            sotf_audio_player_gpui::theme::ThemeId::default(),
        );
        let design = Ds::from_cx(cx);
        let panel = render_lufs_with_true_peak(
            &design,
            Some(&self.loudness),
            1.0,
            sotf_audio_player_gpui::i18n::LevelMeterTranslations::for_language(self.language),
            &theme,
        );
        gpui::div()
            .id("programme-true-peak-render-harness")
            .size_full()
            .child(panel)
            .dev_track("meters.lufs-panel")
    }
}

#[cfg(feature = "dev-api")]
#[gpui::test]
async fn programme_maximum_true_peak_renders_at_compact_width(cx: &mut TestAppContext) {
    use sotf_audio_player_gpui::app::dev_api::registry;
    use sotf_audio_player_gpui::i18n::Language;
    use sotf_plugins::analyzer::{LoudnessRangeData, LoudnessRangeMode, LoudnessRangeStatus};

    let mut loudness = sotf_audio_player::LoudnessData::new(2);
    loudness.maximum_true_peak_dbtp = Some(-0.6);
    loudness.maximum_momentary_lufs = Some(-8.1);
    loudness.maximum_shortterm_lufs = Some(-10.2);
    loudness.loudness_range = Some(LoudnessRangeData {
        range_lu: Some(0.0),
        is_stable: false,
        status: LoudnessRangeStatus::Valid,
        mode: LoudnessRangeMode::Rolling,
        retained_windows: 1,
        observed_windows: 1,
        capacity_windows: 36_000,
        timebase_is_exact: true,
    });
    loudness.true_peak_is_compliant = false;
    loudness.true_peak_valid = false;
    loudness.update_true_peaks(&[-0.6, -0.9]);
    let window: WindowHandle<ProgrammeMaximumTruePeakView> =
        cx.add_window(move |_window, _cx| ProgrammeMaximumTruePeakView {
            loudness,
            language: Language::German,
        });
    cx.simulate_window_resize(window.into(), gpui::size(gpui::px(420.0), gpui::px(720.0)));
    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();
    let window_id = window
        .update(&mut visual_cx, |_, window, _| {
            window.window_handle().window_id().as_u64()
        })
        .unwrap();

    let rendered = tracked_meter_element(window_id, "meters.lufs.maximum-true-peak-value");
    assert_eq!(
        rendered.state.text.as_deref(),
        Some("-0.6 dBTP"),
        "the painted value must preserve the programme maximum"
    );
    let label = tracked_meter_element(window_id, "meters.lufs.maximum-true-peak-label");
    assert_eq!(
        label.state.text.as_deref(),
        Some("Max TP"),
        "the German Max TP label should be present in the painted tree"
    );
    assert_eq!(
        tracked_meter_element(window_id, "meters.lufs.maximum-momentary-label")
            .state
            .text
            .as_deref(),
        Some("Max. Momentan")
    );
    assert_eq!(
        tracked_meter_element(window_id, "meters.lufs.maximum-momentary-value")
            .state
            .text
            .as_deref(),
        Some("-8.1")
    );
    assert_eq!(
        tracked_meter_element(window_id, "meters.lufs.maximum-shortterm-value")
            .state
            .text
            .as_deref(),
        Some("-10.2")
    );
    assert_eq!(
        tracked_meter_element(window_id, "meters.lufs.lra-value")
            .state
            .text
            .as_deref(),
        Some("0.0 LU"),
        "a valid zero LRA remains visible in the compact panel"
    );
    assert_eq!(
        tracked_meter_element(window_id, "meters.lufs.lra-stability")
            .state
            .text
            .as_deref(),
        Some("Noch nicht stabil")
    );
    assert_maximum_true_peak_layout(window_id, "meters.lufs-panel", 420.0, 720.0);
    assert_maximum_lufs_layout(window_id, "meters.lufs-panel", 420.0, 720.0);
    assert_lra_layout(window_id, "meters.lufs-panel", 420.0, 720.0);

    window
        .update(&mut visual_cx, |view, _window, cx| {
            view.loudness.update_true_peaks(&[-3.0, -4.0]);
            cx.notify();
        })
        .unwrap();
    visual_cx.run_until_parked();
    let retained = tracked_meter_element(window_id, "meters.lufs.maximum-true-peak-value");
    assert_eq!(
        retained.state.text.as_deref(),
        Some("-0.6 dBTP"),
        "a lower interval must not replace the rendered programme maximum"
    );

    window
        .update(&mut visual_cx, |view, _window, cx| {
            view.loudness.maximum_true_peak_dbtp = None;
            view.loudness.true_peak_is_compliant = false;
            view.loudness.true_peak_valid = false;
            view.language = Language::German;
            cx.notify();
        })
        .unwrap();
    visual_cx.run_until_parked();
    let unavailable = tracked_meter_element(window_id, "meters.lufs.maximum-true-peak-value");
    assert_eq!(
        unavailable.state.text.as_deref(),
        Some("Nicht verfügbar"),
        "German unsupported snapshots must render the unavailable value"
    );
    assert_maximum_true_peak_layout(window_id, "meters.lufs-panel", 420.0, 720.0);
    assert_maximum_lufs_layout(window_id, "meters.lufs-panel", 420.0, 720.0);

    window
        .update(&mut visual_cx, |_, window, _| window.remove_window())
        .unwrap();
    visual_cx.run_until_parked();
    registry::clear(window_id);
}

#[cfg(feature = "dev-api")]
struct ProgrammeMaximumTruePeakPluginScenario;

#[cfg(feature = "dev-api")]
fn loudness_display_fixture(
    maximum_true_peak: Option<f64>,
    maximum_momentary: Option<f64>,
    maximum_shortterm: Option<f64>,
    channel_peaks: Vec<f64>,
    true_peaks_dbtp: &[f64],
) -> sotf_audio_player::LoudnessData {
    use sotf_audio_player::LoudnessData;
    use sotf_plugins::analyzer::{LoudnessRangeData, LoudnessRangeMode, LoudnessRangeStatus};
    use std::sync::Arc;

    let mut loudness = LoudnessData::new(channel_peaks.len());
    loudness.maximum_true_peak_dbtp = maximum_true_peak;
    loudness.maximum_momentary_lufs = maximum_momentary;
    loudness.maximum_shortterm_lufs = maximum_shortterm;
    loudness.loudness_range = Some(LoudnessRangeData {
        range_lu: Some(0.0),
        is_stable: false,
        status: LoudnessRangeStatus::Valid,
        mode: LoudnessRangeMode::Rolling,
        retained_windows: 1,
        observed_windows: 1,
        capacity_windows: 36_000,
        timebase_is_exact: true,
    });
    loudness.momentary_lufs = -17.0;
    loudness.shortterm_lufs = -18.0;
    loudness.integrated_lufs = -19.0;
    loudness.channel_peaks = Arc::new(channel_peaks);
    loudness.true_peak_is_compliant = false;
    loudness.true_peak_valid = false;
    loudness.momentary_valid = false;
    loudness.shortterm_valid = false;
    loudness.update_true_peaks(true_peaks_dbtp);
    loudness
}

#[cfg(feature = "dev-api")]
fn assert_maximum_lufs_layout(
    window_id: u64,
    panel_selector: &str,
    viewport_width: f32,
    viewport_height: f32,
) {
    use gpui::{Bounds, Pixels};

    let panel = tracked_meter_element(window_id, panel_selector).bounds;
    let momentary =
        tracked_meter_element(window_id, "meters.lufs.maximum-momentary-summary").bounds;
    let shortterm =
        tracked_meter_element(window_id, "meters.lufs.maximum-shortterm-summary").bounds;
    let momentary_label =
        tracked_meter_element(window_id, "meters.lufs.maximum-momentary-label").bounds;
    let momentary_value =
        tracked_meter_element(window_id, "meters.lufs.maximum-momentary-value").bounds;
    let shortterm_label =
        tracked_meter_element(window_id, "meters.lufs.maximum-shortterm-label").bounds;
    let shortterm_value =
        tracked_meter_element(window_id, "meters.lufs.maximum-shortterm-value").bounds;
    let integrated = tracked_meter_element(window_id, "meters.lufs.integrated-bar").bounds;

    let right = |bounds: Bounds<Pixels>| f32::from(bounds.origin.x) + f32::from(bounds.size.width);
    let bottom =
        |bounds: Bounds<Pixels>| f32::from(bounds.origin.y) + f32::from(bounds.size.height);
    assert!(right(panel) <= viewport_width && bottom(panel) <= viewport_height);
    for (name, summary, label, value) in [
        ("momentary", momentary, momentary_label, momentary_value),
        ("short-term", shortterm, shortterm_label, shortterm_value),
    ] {
        assert!(
            summary.origin.x >= panel.origin.x && right(summary) <= right(panel),
            "{name} maximum summary exceeds the panel: {summary:?}, panel {panel:?}"
        );
        assert!(
            summary.origin.y >= panel.origin.y && bottom(summary) <= bottom(panel),
            "{name} maximum summary exceeds the panel vertically: {summary:?}, panel {panel:?}"
        );
        assert!(label.origin.y >= summary.origin.y && bottom(label) <= bottom(summary));
        assert!(value.origin.y >= summary.origin.y && bottom(value) <= bottom(summary));
        assert!(
            bottom(label) <= f32::from(value.origin.y),
            "{name} label/value overlap: {label:?}, {value:?}"
        );
    }
    assert!(
        bottom(momentary) <= f32::from(shortterm.origin.y),
        "M and S maximum rows overlap: {momentary:?}, {shortterm:?}"
    );
    assert!(
        bottom(shortterm) <= f32::from(integrated.origin.y),
        "maximum rows overlap the current integrated bar: {shortterm:?}, {integrated:?}"
    );
}

#[cfg(feature = "dev-api")]
fn assert_lra_layout(
    window_id: u64,
    panel_selector: &str,
    viewport_width: f32,
    viewport_height: f32,
) {
    use gpui::{Bounds, Pixels};

    let panel = tracked_meter_element(window_id, panel_selector).bounds;
    let momentary =
        tracked_meter_element(window_id, "meters.lufs.maximum-momentary-summary").bounds;
    let lra = tracked_meter_element(window_id, "meters.lufs.lra-summary").bounds;
    let lra_label = tracked_meter_element(window_id, "meters.lufs.lra-label").bounds;
    let lra_value = tracked_meter_element(window_id, "meters.lufs.lra-value").bounds;
    let shortterm =
        tracked_meter_element(window_id, "meters.lufs.maximum-shortterm-summary").bounds;
    let integrated = tracked_meter_element(window_id, "meters.lufs.integrated-bar").bounds;

    let right = |bounds: Bounds<Pixels>| f32::from(bounds.origin.x) + f32::from(bounds.size.width);
    let bottom =
        |bounds: Bounds<Pixels>| f32::from(bounds.origin.y) + f32::from(bounds.size.height);
    assert!(right(panel) <= viewport_width && bottom(panel) <= viewport_height);
    for (name, bounds) in [
        ("LRA summary", lra),
        ("LRA label", lra_label),
        ("LRA value", lra_value),
    ] {
        assert!(
            bounds.origin.x >= panel.origin.x && right(bounds) <= right(panel),
            "{name} exceeds the panel width: {bounds:?}, panel {panel:?}"
        );
        assert!(
            bounds.origin.y >= panel.origin.y && bottom(bounds) <= bottom(panel),
            "{name} exceeds the panel height: {bounds:?}, panel {panel:?}"
        );
    }
    assert!(
        bottom(lra_label) <= f32::from(lra_value.origin.y),
        "LRA label and value overlap: {lra_label:?}, {lra_value:?}"
    );
    assert!(
        bottom(momentary) <= f32::from(lra.origin.y),
        "LRA summary overlaps the M maximum: {momentary:?}, {lra:?}"
    );
    assert!(
        bottom(lra) <= f32::from(shortterm.origin.y),
        "LRA summary overlaps the S maximum: {lra:?}, {shortterm:?}"
    );
    assert!(
        bottom(shortterm) <= f32::from(integrated.origin.y),
        "LRA/M/S summaries overlap the current integrated bar: {shortterm:?}, {integrated:?}"
    );
}

#[cfg(feature = "dev-api")]
impl TestScenario for ProgrammeMaximumTruePeakPluginScenario {
    fn name(&self) -> &'static str {
        "Programme M/S and true-peak maxima are visible in the Loudness Monitor plugin"
    }

    fn window_size(&self) -> Option<gpui::Size<gpui::Pixels>> {
        Some(gpui::size(gpui::px(720.0), gpui::px(1100.0)))
    }

    fn execute(
        &self,
        cx: &mut VisualTestContext,
        window: WindowHandle<PlayerView>,
    ) -> Result<(), Box<dyn Error>> {
        use crate::driver::AppDriver;
        use sotf_audio::plugins::PluginType;
        use sotf_audio_player_gpui::Screen;
        use sotf_audio_player_gpui::i18n::Language;
        use std::sync::Arc;

        let mut driver = AppDriver::new(cx, window);
        let initial = loudness_display_fixture(
            Some(-0.6),
            Some(-8.1),
            Some(-10.2),
            vec![0.5, 0.4],
            &[-0.6, -0.9],
        );
        driver.update_app(move |app, cx| {
            app.ui_state.current_screen = Screen::Studio;
            app.set_language(Language::German);
            app.plugin_state.add_plugin(&PluginType::LoudnessMonitor);
            app.playback.loudness_info = Some(Arc::new(initial.clone()));
            app.playback.qa_loudness_fixture = Some(Arc::new(initial));
            cx.notify();
        });
        driver.run_until_parked();

        let window_id = driver
            .view
            .update(driver.cx, |_, window, _| {
                window.window_handle().window_id().as_u64()
            })
            .unwrap();
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.maximum-true-peak-value")
                .state
                .text
                .as_deref(),
            Some("-0.6 dBTP")
        );
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.maximum-momentary-label")
                .state
                .text
                .as_deref(),
            Some("Max. Momentan")
        );
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.maximum-momentary-value")
                .state
                .text
                .as_deref(),
            Some("-8.1"),
            "the finite M maximum is displayed despite an invalid current M window"
        );
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.maximum-shortterm-value")
                .state
                .text
                .as_deref(),
            Some("-10.2"),
            "the finite S maximum is displayed despite an invalid current S window"
        );
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.lra-label")
                .state
                .text
                .as_deref(),
            Some("LRA")
        );
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.lra-value")
                .state
                .text
                .as_deref(),
            Some("0.0 LU"),
            "zero is a valid numeric LRA"
        );
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.lra-stability")
                .state
                .text
                .as_deref(),
            Some("Noch nicht stabil")
        );
        assert_maximum_true_peak_layout(window_id, "plugin.loudness-monitor.panel", 720.0, 1100.0);
        assert_maximum_lufs_layout(window_id, "plugin.loudness-monitor.panel", 720.0, 1100.0);
        assert_lra_layout(window_id, "plugin.loudness-monitor.panel", 720.0, 1100.0);

        // Only the maximum M/S fields change. The current readings and all
        // other fixture values remain fixed, so the mounted view must react
        // to the programme summaries themselves.
        let maximum_only = loudness_display_fixture(
            Some(-0.6),
            Some(-7.9),
            Some(-9.9),
            vec![0.5, 0.4],
            &[-0.6, -0.9],
        );
        driver.update_app(move |app, cx| {
            app.playback.loudness_info = Some(Arc::new(maximum_only.clone()));
            app.playback.qa_loudness_fixture = Some(Arc::new(maximum_only));
            cx.notify();
        });
        driver.run_until_parked();
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.maximum-momentary-value")
                .state
                .text
                .as_deref(),
            Some("-7.9")
        );
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.maximum-shortterm-value")
                .state
                .text
                .as_deref(),
            Some("-9.9")
        );

        let mut stable = loudness_display_fixture(
            Some(-0.6),
            Some(-7.9),
            Some(-9.9),
            vec![0.5, 0.4],
            &[-0.6, -0.9],
        );
        stable.loudness_range.as_mut().unwrap().is_stable = true;
        driver.update_app(move |app, cx| {
            app.playback.loudness_info = Some(Arc::new(stable.clone()));
            app.playback.qa_loudness_fixture = Some(Arc::new(stable));
            cx.notify();
        });
        driver.run_until_parked();
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.lra-value")
                .state
                .text
                .as_deref(),
            Some("0.0 LU")
        );
        assert!(
            try_tracked_meter_element(window_id, "meters.lufs.lra-stability").is_none(),
            "stable LRA must not retain the warming-up marker"
        );
        assert_lra_layout(window_id, "plugin.loudness-monitor.panel", 720.0, 1100.0);

        let higher = loudness_display_fixture(
            Some(-0.4),
            Some(-7.2),
            Some(-9.1),
            vec![0.7, 0.6],
            &[-0.4, -0.7],
        );
        driver.update_app(move |app, cx| {
            app.playback.loudness_info = Some(Arc::new(higher.clone()));
            app.playback.qa_loudness_fixture = Some(Arc::new(higher));
            cx.notify();
        });
        driver.run_until_parked();
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.maximum-momentary-value")
                .state
                .text
                .as_deref(),
            Some("-7.2")
        );
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.maximum-shortterm-value")
                .state
                .text
                .as_deref(),
            Some("-9.1")
        );

        let lower = loudness_display_fixture(
            Some(-0.4),
            Some(-7.2),
            Some(-9.1),
            vec![0.25, 0.2],
            &[-3.0, -4.0],
        );
        driver.update_app(move |app, cx| {
            app.playback.loudness_info = Some(Arc::new(lower.clone()));
            app.playback.qa_loudness_fixture = Some(Arc::new(lower));
            cx.notify();
        });
        driver.run_until_parked();
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.maximum-momentary-value")
                .state
                .text
                .as_deref(),
            Some("-7.2"),
            "a lower interval cannot replace the M latch"
        );
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.maximum-shortterm-value")
                .state
                .text
                .as_deref(),
            Some("-9.1"),
            "a lower interval cannot replace the S latch"
        );

        let reset = loudness_display_fixture(None, None, None, vec![0.25, 0.2], &[-3.0, -4.0]);
        driver.update_app(move |app, cx| {
            app.playback.loudness_info = Some(Arc::new(reset.clone()));
            app.playback.qa_loudness_fixture = Some(Arc::new(reset));
            cx.notify();
        });
        driver.run_until_parked();
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.maximum-momentary-value")
                .state
                .text
                .as_deref(),
            Some("—")
        );
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.maximum-shortterm-value")
                .state
                .text
                .as_deref(),
            Some("—")
        );
        let mut below_gate = loudness_display_fixture(
            Some(-0.6),
            Some(-7.9),
            Some(-9.9),
            vec![0.5, 0.4],
            &[-0.6, -0.9],
        );
        if let Some(range) = below_gate.loudness_range.as_mut() {
            range.range_lu = None;
            range.is_stable = true;
            range.status = sotf_plugins::analyzer::LoudnessRangeStatus::BelowGate;
        }
        driver.update_app(move |app, cx| {
            app.playback.loudness_info = Some(Arc::new(below_gate.clone()));
            app.playback.qa_loudness_fixture = Some(Arc::new(below_gate));
            cx.notify();
        });
        driver.run_until_parked();
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.lra-value")
                .state
                .text
                .as_deref(),
            Some("Nicht verfügbar")
        );
        assert!(
            try_tracked_meter_element(window_id, "meters.lufs.lra-stability").is_none(),
            "unavailable LRA must not show a stability claim"
        );

        let mut malformed = loudness_display_fixture(
            Some(-0.6),
            Some(-7.9),
            Some(-9.9),
            vec![0.5, 0.4],
            &[-0.6, -0.9],
        );
        if let Some(range) = malformed.loudness_range.as_mut() {
            range.range_lu = Some(f64::INFINITY);
            range.is_stable = true;
        }
        driver.update_app(move |app, cx| {
            app.playback.loudness_info = Some(Arc::new(malformed.clone()));
            app.playback.qa_loudness_fixture = Some(Arc::new(malformed));
            cx.notify();
        });
        driver.run_until_parked();
        assert_eq!(
            tracked_meter_element(window_id, "meters.lufs.lra-value")
                .state
                .text
                .as_deref(),
            Some("Nicht verfügbar"),
            "malformed nonfinite LRA must fail closed even with Valid status"
        );
        assert!(try_tracked_meter_element(window_id, "meters.lufs.lra-stability").is_none());
        assert_maximum_lufs_layout(window_id, "plugin.loudness-monitor.panel", 720.0, 1100.0);
        assert_lra_layout(window_id, "plugin.loudness-monitor.panel", 720.0, 1100.0);
        Ok(())
    }
}

#[cfg(feature = "dev-api")]
#[gpui::test]
async fn programme_maximum_true_peak_renders_in_mounted_loudness_plugin(cx: &mut TestAppContext) {
    let runner = E2ERunner::new(ProgrammeMaximumTruePeakPluginScenario);
    let result = runner.run(cx).await.unwrap();
    assert!(
        result.passed,
        "{}",
        result.error_message.unwrap_or_default()
    );
}

#[cfg(feature = "dev-api")]
fn assert_meter_query_route(
    endpoint: std::net::SocketAddr,
    visual_cx: &mut VisualTestContext,
    path: &'static str,
    expected: serde_json::Value,
) {
    let actual = read_meter_query_route(endpoint, visual_cx, path);
    assert_eq!(actual, expected, "query path {path}");
}

#[cfg(feature = "dev-api")]
fn read_meter_query_route(
    endpoint: std::net::SocketAddr,
    visual_cx: &mut VisualTestContext,
    path: &'static str,
) -> serde_json::Value {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::{Duration, Instant};

    const RUN_ID: &str = "0123456789abcdef0123456789abcdef";
    let response_thread = std::thread::spawn(move || {
        let mut stream = TcpStream::connect(endpoint).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        write!(
            stream,
            "GET /query?path={path} HTTP/1.1\r\nHost: localhost\r\nX-SOTF-Dev-Run-ID: {RUN_ID}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while !response_thread.is_finished() {
        visual_cx
            .executor()
            .advance_clock(Duration::from_millis(25));
        visual_cx.run_until_parked();
        assert!(
            Instant::now() < deadline,
            "the live dev API route did not return a response for {path}"
        );
    }
    let response = response_thread.join().unwrap();
    let (status, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(status.starts_with("HTTP/1.1 200"), "{status}");
    let body: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["ok"], serde_json::json!(true));
    body["value"].clone()
}

#[cfg(feature = "dev-api")]
fn click_meter_control_route(
    endpoint: std::net::SocketAddr,
    visual_cx: &mut VisualTestContext,
    selector: String,
) {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::{Duration, Instant};

    const RUN_ID: &str = "0123456789abcdef0123456789abcdef";
    let body = serde_json::json!({ "selector": selector }).to_string();
    let response_thread = std::thread::spawn(move || {
        let mut stream = TcpStream::connect(endpoint).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        write!(
            stream,
            "POST /click HTTP/1.1\r\nHost: localhost\r\nX-SOTF-Dev-Run-ID: {RUN_ID}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while !response_thread.is_finished() {
        visual_cx
            .executor()
            .advance_clock(Duration::from_millis(25));
        visual_cx.run_until_parked();
        assert!(
            Instant::now() < deadline,
            "the live dev API click route timed out"
        );
    }
    let response = response_thread.join().unwrap();
    let (status, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(status.starts_with("HTTP/1.1 200"), "{status}");
    let body: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["ok"], serde_json::json!(true));
}

#[cfg(feature = "dev-api")]
#[gpui::test]
async fn programme_maximum_loudness_queries_resolve_live_app_state(cx: &mut TestAppContext) {
    use gpui::AppContext;
    use sotf_audio::plugins::PluginType;
    use sotf_audio_player::Player;
    use sotf_audio_player::plugin_graph::NodeRole;
    use sotf_audio_player_gpui::Screen;
    use sotf_audio_player_gpui::app::dev_api;
    use sotf_audio_player_gpui::app::player_handle::PlayerHandle;
    use sotf_audio_player_gpui::app::state::plugin::LoudnessControlUiRequest;
    use sotf_audio_player_gpui::app::state::ui::LayoutState;
    use sotf_audio_player_gpui::app::{App, AppState};
    use std::sync::Arc;

    const RUN_ID: &str = "0123456789abcdef0123456789abcdef";
    let window = cx.add_window(|_, cx| {
        let state = cx.new(|cx| {
            let mut app = App::new();
            app.ui_state.current_screen = Screen::Studio;
            app.plugin_state.add_plugin(&PluginType::LoudnessMonitor);
            let player = Player::new();
            let layout = cx.new(|_| LayoutState::default());
            AppState {
                app,
                layout,
                player: PlayerHandle::new(player),
            }
        });
        PlayerView::new(state, cx)
    });
    let mut initial = loudness_display_fixture(
        Some(-1.2),
        Some(-8.2),
        Some(-10.4),
        vec![0.5, 0.4],
        &[-3.0, -4.0],
    );
    initial.integrated_control_instance_id = 77;
    initial.integrated_control_request_id = 0;
    initial.integrated_measurement_running = false;
    window
        .update(cx, |view, _, cx| {
            view.state.update(cx, |state, _| {
                state.app.playback.loudness_info = Some(Arc::new(initial.clone()));
                state.app.playback.qa_loudness_fixture = Some(Arc::new(initial));
            })
        })
        .unwrap();

    let any_window = window.clone().into();
    let server = cx
        .update(|cx| dev_api::start_for_testing(cx, 0, any_window, RUN_ID))
        .unwrap();
    let endpoint = server.endpoint();
    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    visual_cx.run_until_parked();
    let check = |visual_cx: &mut VisualTestContext, tp, momentary, shortterm| {
        assert_meter_query_route(endpoint, visual_cx, "meters.maximum_true_peak_dbtp", tp);
        assert_meter_query_route(
            endpoint,
            visual_cx,
            "meters.maximum_momentary_lufs",
            momentary,
        );
        assert_meter_query_route(
            endpoint,
            visual_cx,
            "meters.maximum_shortterm_lufs",
            shortterm,
        );
    };
    check(
        &mut visual_cx,
        serde_json::json!(-1.2),
        serde_json::json!(-8.2),
        serde_json::json!(-10.4),
    );

    for (fixture, expected_tp, expected_m, expected_s) in [
        (
            loudness_display_fixture(
                Some(-0.8),
                Some(-7.4),
                Some(-9.3),
                vec![0.7, 0.5],
                &[-0.8, -1.0],
            ),
            serde_json::json!(-0.8),
            serde_json::json!(-7.4),
            serde_json::json!(-9.3),
        ),
        (
            loudness_display_fixture(
                Some(-0.8),
                Some(-7.4),
                Some(-9.3),
                vec![0.25, 0.2],
                &[-3.0, -4.0],
            ),
            serde_json::json!(-0.8),
            serde_json::json!(-7.4),
            serde_json::json!(-9.3),
        ),
        (
            loudness_display_fixture(None, None, None, vec![0.25, 0.2], &[-3.0, -4.0]),
            serde_json::Value::Null,
            serde_json::Value::Null,
            serde_json::Value::Null,
        ),
    ] {
        window
            .update(&mut visual_cx, |view, _, cx| {
                view.state.update(cx, |state, _| {
                    state.app.playback.loudness_info = Some(Arc::new(fixture.clone()));
                    state.app.playback.qa_loudness_fixture = Some(Arc::new(fixture));
                })
            })
            .unwrap();
        visual_cx.run_until_parked();
        check(&mut visual_cx, expected_tp, expected_m, expected_s);
    }

    let nonfinite = loudness_display_fixture(
        Some(f64::INFINITY),
        Some(f64::INFINITY),
        Some(f64::NAN),
        vec![0.25, 0.2],
        &[-3.0, -4.0],
    );
    window
        .update(&mut visual_cx, |view, _, cx| {
            view.state.update(cx, |state, _| {
                state.app.playback.loudness_info = Some(Arc::new(nonfinite.clone()));
                state.app.playback.qa_loudness_fixture = Some(Arc::new(nonfinite));
            })
        })
        .unwrap();
    visual_cx.run_until_parked();
    check(
        &mut visual_cx,
        serde_json::Value::Null,
        serde_json::Value::Null,
        serde_json::Value::Null,
    );

    let mut control_fixture = loudness_display_fixture(
        Some(-0.8),
        Some(-7.4),
        Some(-9.3),
        vec![0.7, 0.5],
        &[-0.8, -1.0],
    );
    control_fixture.integrated_control_instance_id = 77;
    control_fixture.integrated_control_request_id = 0;
    control_fixture.integrated_measurement_running = false;
    window
        .update(&mut visual_cx, |view, _, cx| {
            view.state.update(cx, |state, cx| {
                state.app.ui_state.current_screen = Screen::Studio;
                state.app.playback.loudness_info = Some(Arc::new(control_fixture.clone()));
                state.app.playback.qa_loudness_fixture = Some(Arc::new(control_fixture.clone()));
                cx.notify();
            })
        })
        .unwrap();
    visual_cx.run_until_parked();
    let (visible_graph_id, output_graph_id, output_engine_index, input_engine_index) = window
        .update(&mut visual_cx, |view, _, cx| {
            let state = view.state.read(cx);
            let graph = &state.app.plugin_state.graph;
            let output_id = graph
                .node_for_role(NodeRole::OutputMonitor)
                .expect("default rack has output analyzer")
                .plugin
                .id;
            let input_id = graph
                .node_for_role(NodeRole::InputMonitor)
                .expect("default rack has input analyzer")
                .plugin
                .id;
            let visible_id = graph
                .plugins()
                .into_iter()
                .find(|plugin| {
                    plugin.plugin_type() == PluginType::LoudnessMonitor
                        && plugin.id != output_id
                        && plugin.id != input_id
                })
                .expect("fixture inserted one visible loudness monitor")
                .id;
            (
                visible_id,
                output_id,
                graph.output_monitor_engine_index().unwrap(),
                graph.input_monitor_engine_index().unwrap(),
            )
        })
        .unwrap();
    assert_ne!(visible_graph_id, output_graph_id);
    assert_ne!(input_engine_index, output_engine_index);
    let window_id = window
        .update(&mut visual_cx, |_, window, _| {
            window.window_handle().window_id().as_u64()
        })
        .unwrap();
    let output_pause = format!("plugin.loudness.control.{visible_graph_id}.pause");
    assert_eq!(
        tracked_meter_element(window_id, &output_pause)
            .state
            .enabled,
        Some(true),
        "the visible analyzer panel exposes controls for its displayed output snapshot"
    );
    click_meter_control_route(endpoint, &mut visual_cx, output_pause);
    assert_meter_query_route(
        endpoint,
        &mut visual_cx,
        "meters.integrated_control_requests",
        serde_json::json!([{
            "plugin_id": visible_graph_id,
            "runtime_instance_id": 77,
            "request_id": 1,
            "engine_index": output_engine_index,
            "operation": "pause",
            "command": "77:1:pause",
            "error": null,
        }]),
    );
    let output_status = format!("plugin.loudness.control-status.{visible_graph_id}");
    assert_eq!(
        tracked_meter_element(window_id, &output_status)
            .state
            .text
            .as_deref(),
        Some("Pause I/LRA · Waiting for meter update")
    );

    // A delayed older receipt cannot complete a newer reset. Reset remains a
    // valid explicit action while the current I/LRA state is paused.
    let output_reset = format!("plugin.loudness.control.{visible_graph_id}.reset");
    click_meter_control_route(endpoint, &mut visual_cx, output_reset);
    assert_meter_query_route(
        endpoint,
        &mut visual_cx,
        "meters.integrated_control_requests",
        serde_json::json!([{
            "plugin_id": visible_graph_id,
            "runtime_instance_id": 77,
            "request_id": 2,
            "engine_index": output_engine_index,
            "operation": "reset",
            "command": "77:2:reset",
            "error": null,
        }]),
    );
    control_fixture.integrated_control_request_id = 1;
    window
        .update(&mut visual_cx, |view, _, cx| {
            view.state.update(cx, |state, cx| {
                state.app.playback.loudness_info = Some(Arc::new(control_fixture.clone()));
                state.app.playback.qa_loudness_fixture = Some(Arc::new(control_fixture.clone()));
                cx.notify();
            })
        })
        .unwrap();
    visual_cx.run_until_parked();
    assert_eq!(
        tracked_meter_element(window_id, &output_status)
            .state
            .text
            .as_deref(),
        Some("Reset I/LRA · Waiting for meter update"),
        "the older pause receipt does not acknowledge reset"
    );
    control_fixture.integrated_control_request_id = 2;
    control_fixture.integrated_measurement_running = false;
    window
        .update(&mut visual_cx, |view, _, cx| {
            view.state.update(cx, |state, cx| {
                state.app.playback.loudness_info = Some(Arc::new(control_fixture.clone()));
                state.app.playback.qa_loudness_fixture = Some(Arc::new(control_fixture.clone()));
                cx.notify();
            })
        })
        .unwrap();
    visual_cx.run_until_parked();
    assert_eq!(
        tracked_meter_element(window_id, &output_status)
            .state
            .text
            .as_deref(),
        Some("I/LRA paused")
    );
    let output_retry = format!("plugin.loudness.control.retry.{visible_graph_id}");
    assert_eq!(
        tracked_meter_element(window_id, &output_retry)
            .state
            .enabled,
        Some(false),
        "an exactly acknowledged reset is terminal and cannot be retried"
    );

    let output_start = format!("plugin.loudness.control.{visible_graph_id}.start");
    click_meter_control_route(endpoint, &mut visual_cx, output_start);
    assert_meter_query_route(
        endpoint,
        &mut visual_cx,
        "meters.integrated_control_requests",
        serde_json::json!([{
            "plugin_id": visible_graph_id,
            "runtime_instance_id": 77,
            "request_id": 3,
            "engine_index": output_engine_index,
            "operation": "start",
            "command": "77:3:start",
            "error": null,
        }]),
    );
    control_fixture.integrated_control_request_id = 3;
    control_fixture.integrated_measurement_running = true;
    window
        .update(&mut visual_cx, |view, _, cx| {
            view.state.update(cx, |state, cx| {
                state.app.playback.loudness_info = Some(Arc::new(control_fixture.clone()));
                state.app.playback.qa_loudness_fixture = Some(Arc::new(control_fixture.clone()));
                cx.notify();
            })
        })
        .unwrap();
    visual_cx.run_until_parked();
    assert_eq!(
        tracked_meter_element(window_id, &output_status)
            .state
            .text
            .as_deref(),
        Some("I/LRA running")
    );

    let output_continue = format!("plugin.loudness.control.{visible_graph_id}.continue");
    click_meter_control_route(endpoint, &mut visual_cx, output_continue);
    assert_meter_query_route(
        endpoint,
        &mut visual_cx,
        "meters.integrated_control_requests",
        serde_json::json!([{
            "plugin_id": visible_graph_id,
            "runtime_instance_id": 77,
            "request_id": 4,
            "engine_index": output_engine_index,
            "operation": "continue",
            "command": "77:4:continue",
            "error": null,
        }]),
    );
    control_fixture.integrated_control_request_id = 4;
    control_fixture.integrated_measurement_running = true;
    window
        .update(&mut visual_cx, |view, _, cx| {
            view.state.update(cx, |state, cx| {
                state.app.playback.loudness_info = Some(Arc::new(control_fixture.clone()));
                state.app.playback.qa_loudness_fixture = Some(Arc::new(control_fixture.clone()));
                cx.notify();
            })
        })
        .unwrap();
    visual_cx.run_until_parked();
    assert_eq!(
        tracked_meter_element(window_id, &output_status)
            .state
            .text
            .as_deref(),
        Some("I/LRA running"),
        "Continue while already running is acknowledged as a no-op"
    );

    // A rebuilt analyzer has a different runtime identity. The old request
    // cannot appear acknowledged; an explicit retry targets the new identity.
    control_fixture.integrated_control_instance_id = 88;
    control_fixture.integrated_control_request_id = 0;
    control_fixture.integrated_measurement_running = false;
    window
        .update(&mut visual_cx, |view, _, cx| {
            view.state.update(cx, |state, cx| {
                state.app.playback.loudness_info = Some(Arc::new(control_fixture.clone()));
                state.app.playback.qa_loudness_fixture = Some(Arc::new(control_fixture.clone()));
                cx.notify();
            })
        })
        .unwrap();
    visual_cx.run_until_parked();
    assert_eq!(
        tracked_meter_element(window_id, &output_status)
            .state
            .text
            .as_deref(),
        Some("Monitor changed; request not confirmed")
    );
    assert_eq!(
        tracked_meter_element(window_id, &output_retry)
            .state
            .enabled,
        Some(false),
        "a request for the old analyzer cannot be retried against its replacement"
    );
    window
        .update(&mut visual_cx, |view, _, cx| {
            view.state.update(cx, |state, cx| {
                state.app.playback.loudness_info = None;
                state.app.playback.qa_loudness_fixture = None;
                cx.notify();
            })
        })
        .unwrap();
    visual_cx.run_until_parked();
    assert_eq!(
        tracked_meter_element(window_id, &output_status)
            .state
            .text
            .as_deref(),
        Some("Monitor changed; request not confirmed"),
        "snapshot loss does not hide cancellation of the old request"
    );
    assert_eq!(
        tracked_meter_element(window_id, &output_retry)
            .state
            .enabled,
        Some(false)
    );
    window
        .update(&mut visual_cx, |view, _, cx| {
            view.state.update(cx, |state, cx| {
                state.app.playback.loudness_info = Some(Arc::new(control_fixture.clone()));
                state.app.playback.qa_loudness_fixture = Some(Arc::new(control_fixture.clone()));
                cx.notify();
            })
        })
        .unwrap();
    visual_cx.run_until_parked();
    let replacement_reset = format!("plugin.loudness.control.{visible_graph_id}.reset");
    click_meter_control_route(endpoint, &mut visual_cx, replacement_reset);
    assert_meter_query_route(
        endpoint,
        &mut visual_cx,
        "meters.integrated_control_requests",
        serde_json::json!([{
            "plugin_id": visible_graph_id,
            "runtime_instance_id": 88,
            "request_id": 5,
            "engine_index": output_engine_index,
            "operation": "reset",
            "command": "88:5:reset",
            "error": null,
        }]),
    );

    // A late exact host publication supersedes a previously reported transport
    // failure. The receipt is authoritative once it is visible on this same
    // runtime instance.
    let late_ack = LoudnessControlUiRequest {
        runtime_instance_id: 88,
        request_id: 6,
        engine_index: output_engine_index,
        operation: "pause",
        command: "88:6:pause".to_string(),
        error: Some("temporary submission failure".to_string()),
    };
    control_fixture.integrated_control_instance_id = 88;
    control_fixture.integrated_control_request_id = 6;
    control_fixture.integrated_measurement_running = false;
    window
        .update(&mut visual_cx, |view, _, cx| {
            view.state.update(cx, |state, cx| {
                state
                    .app
                    .plugin_state
                    .plugin_ui_state
                    .loudness_control_requests
                    .insert(visible_graph_id, late_ack);
                state.app.playback.loudness_info = Some(Arc::new(control_fixture.clone()));
                state.app.playback.qa_loudness_fixture = Some(Arc::new(control_fixture.clone()));
                cx.notify();
            })
        })
        .unwrap();
    visual_cx.run_until_parked();
    assert_eq!(
        tracked_meter_element(window_id, &output_status)
            .state
            .text
            .as_deref(),
        Some("I/LRA paused"),
        "an exact receipt wins over an earlier submission error"
    );
    assert_eq!(
        tracked_meter_element(window_id, &output_retry)
            .state
            .enabled,
        Some(false)
    );

    // If a failed request's analyzer is replaced before the next snapshot,
    // the UI reports replacement instead of keeping the old transport error.
    let replaced_error = LoudnessControlUiRequest {
        runtime_instance_id: 88,
        request_id: 7,
        engine_index: output_engine_index,
        operation: "reset",
        command: "88:7:reset".to_string(),
        error: Some("temporary submission failure".to_string()),
    };
    control_fixture.integrated_control_instance_id = 89;
    control_fixture.integrated_control_request_id = 0;
    window
        .update(&mut visual_cx, |view, _, cx| {
            view.state.update(cx, |state, cx| {
                state
                    .app
                    .plugin_state
                    .plugin_ui_state
                    .loudness_control_requests
                    .insert(visible_graph_id, replaced_error);
                state.app.playback.loudness_info = Some(Arc::new(control_fixture.clone()));
                state.app.playback.qa_loudness_fixture = Some(Arc::new(control_fixture.clone()));
                cx.notify();
            })
        })
        .unwrap();
    visual_cx.run_until_parked();
    assert_eq!(
        tracked_meter_element(window_id, &output_status)
            .state
            .text
            .as_deref(),
        Some("Monitor changed; request not confirmed"),
        "runtime replacement is more authoritative than a stale submission error"
    );
    assert_eq!(
        tracked_meter_element(window_id, &output_retry)
            .state
            .enabled,
        Some(false)
    );

    // Bridge the mounted UI command payload to two real host instances. The
    // GPUI PlayerHandle is a test double here; this applies the exact payload
    // it recorded to the host selected by the reported output engine index,
    // then returns that host's own published snapshot to the panel.
    use sotf_plugins::{LoudnessMonitorPlugin, ParameterId, ParameterValue, Plugin};
    const RATE: u32 = 48_000;
    let mut input_host = LoudnessMonitorPlugin::new(2).unwrap();
    input_host.initialize(RATE).unwrap();
    let mut output_host = LoudnessMonitorPlugin::new(2).unwrap();
    output_host.initialize(RATE).unwrap();
    let initial_input: Arc<sotf_audio_player::LoudnessData> =
        input_host.get_data().unwrap().downcast().unwrap();
    let actual_input_id = initial_input.integrated_control_instance_id;
    let initial_output: Arc<sotf_audio_player::LoudnessData> =
        output_host.get_data().unwrap().downcast().unwrap();
    let actual_output_id = initial_output.integrated_control_instance_id;
    assert_ne!(actual_input_id, actual_output_id);
    assert_ne!(actual_output_id, 0);
    assert_eq!(initial_output.integrated_control_request_id, 0);

    window
        .update(&mut visual_cx, |view, _, cx| {
            view.state.update(cx, |state, cx| {
                state.app.playback.loudness_info = Some(initial_output.clone());
                state.app.playback.qa_loudness_fixture = Some(initial_output.clone());
                cx.notify();
            })
        })
        .unwrap();
    visual_cx.run_until_parked();

    for (operation, expected_running) in [
        ("start", true),
        ("pause", false),
        ("reset", false),
        ("continue", true),
    ] {
        let selector = format!("plugin.loudness.control.{visible_graph_id}.{operation}");
        click_meter_control_route(endpoint, &mut visual_cx, selector);
        let requests = read_meter_query_route(
            endpoint,
            &mut visual_cx,
            "meters.integrated_control_requests",
        );
        let request = requests
            .as_array()
            .expect("the UI query returns the current request map")
            .iter()
            .find(|request| request["plugin_id"].as_u64() == Some(visible_graph_id as u64))
            .expect("the visible panel records its submitted host command");
        let routed_engine_index = request["engine_index"]
            .as_u64()
            .expect("the UI command contains its selected engine index")
            as usize;
        assert_eq!(routed_engine_index, output_engine_index);
        assert_ne!(routed_engine_index, input_engine_index);
        assert_eq!(
            request["runtime_instance_id"].as_u64(),
            Some(actual_output_id)
        );
        assert_eq!(request["operation"].as_str(), Some(operation));
        assert_eq!(request["error"], serde_json::Value::Null);
        let request_id = request["request_id"]
            .as_u64()
            .expect("the UI command contains a monotonic request ID");
        let command = request["command"]
            .as_str()
            .expect("the UI command contains the host payload")
            .to_owned();
        assert_eq!(
            command,
            format!("{actual_output_id}:{request_id}:{operation}")
        );

        output_host
            .set_parameter(
                ParameterId::from("integrated_control_command".to_string()),
                ParameterValue::String(command),
            )
            .unwrap();
        let published: Arc<sotf_audio_player::LoudnessData> =
            output_host.get_data().unwrap().downcast().unwrap();
        assert_eq!(published.integrated_control_instance_id, actual_output_id);
        assert_eq!(published.integrated_control_request_id, request_id);
        assert_eq!(published.integrated_measurement_running, expected_running);

        // The real host snapshot, including its receipt, is now the snapshot
        // rendered and queried by the mounted panel.
        window
            .update(&mut visual_cx, |view, _, cx| {
                view.state.update(cx, |state, cx| {
                    state.app.playback.loudness_info = Some(published.clone());
                    state.app.playback.qa_loudness_fixture = Some(published.clone());
                    cx.notify();
                })
            })
            .unwrap();
        visual_cx.run_until_parked();
        assert_eq!(
            tracked_meter_element(window_id, &output_status)
                .state
                .text
                .as_deref(),
            Some(if expected_running {
                "I/LRA running"
            } else {
                "I/LRA paused"
            }),
            "the panel reflects the actual output host receipt for {operation}"
        );
        assert_meter_query_route(
            endpoint,
            &mut visual_cx,
            "meters.integrated_control_requests",
            serde_json::json!([{
                "plugin_id": visible_graph_id,
                "runtime_instance_id": actual_output_id,
                "request_id": request_id,
                "engine_index": output_engine_index,
                "operation": operation,
                "command": format!("{actual_output_id}:{request_id}:{operation}"),
                "error": null,
            }]),
        );

        let input_snapshot: Arc<sotf_audio_player::LoudnessData> =
            input_host.get_data().unwrap().downcast().unwrap();
        assert_eq!(
            input_snapshot.integrated_control_instance_id,
            actual_input_id
        );
        assert_eq!(input_snapshot.integrated_control_request_id, 0);
        assert!(
            input_snapshot.integrated_measurement_running,
            "output controls must not change the independent input monitor"
        );
    }

    server.shutdown();
    window
        .update(&mut visual_cx, |_, window, _| window.remove_window())
        .unwrap();
    visual_cx.run_until_parked();
    sotf_audio_player_gpui::app::dev_api::registry::clear(window_id);
}
