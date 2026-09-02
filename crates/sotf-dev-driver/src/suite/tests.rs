use super::misc::safe_name;
use super::types::SuiteFile;

#[test]
fn parses_suite_with_scenario_alias() {
    let src = r#"
            [runner]
            app_bin = "target/debug/sotf-desktop"
            phone_layout = true

            [[scenario]]
            name = "smoke"
            path = "crates/sotf-dev-driver/scenarios/smoke.scn"
            seed_demo_audio = true
            tags = ["smoke"]

[scenario.fake_recording]
channels = 2
points = 48
fault = "clipping"

            [scenario.room_eq]
            fixture_dir = "crates/sotf-dev-driver/testkit/roomeq/stereo_reference"
            dist_path = "fixtures/roomeq/stereo_reference"
            target = "NearField"
            loss = "Flat"
            processing = "Iir"
            crossover = "Lr24"
            num_filters = 7
            max_iter = 20
            population = 24
        "#;
    let suite: SuiteFile = toml::from_str(src).unwrap();
    assert!(suite.runner.phone_layout);
    assert_eq!(suite.scenarios.len(), 1);
    assert_eq!(suite.scenarios[0].name, "smoke");
    assert!(suite.scenarios[0].seed_demo_audio);
    let fake = suite.scenarios[0].fake_recording.as_ref().unwrap();
    assert_eq!(fake.channels, 2);
    assert_eq!(fake.points, 48);
    assert_eq!(fake.fault.as_deref(), Some("clipping"));
    let room_eq = suite.scenarios[0].room_eq.as_ref().unwrap();
    assert_eq!(room_eq.target, "NearField");
    assert_eq!(room_eq.loss, "Flat");
    assert_eq!(room_eq.processing, "Iir");
    assert_eq!(room_eq.crossover, "Lr24");
    assert_eq!(room_eq.num_filters, 7);
    assert_eq!(room_eq.max_iter, 20);
    assert_eq!(room_eq.population, 24);
    assert!(room_eq.start);
}

#[test]
fn safe_name_removes_path_punctuation() {
    assert_eq!(safe_name("Player / Smoke"), "Player---Smoke");
}

#[test]
fn checked_in_suites_parse() {
    let smoke: SuiteFile = toml::from_str(include_str!("../../suites/smoke.toml")).unwrap();
    assert!(!smoke.scenarios.is_empty());

    let roomeq: SuiteFile =
        toml::from_str(include_str!("../../suites/roomeq_matrix.toml")).unwrap();
    assert_eq!(roomeq.scenarios.len(), 24);

    let tui: SuiteFile = toml::from_str(include_str!("../../suites/tui.toml")).unwrap();
    assert_eq!(tui.scenarios.len(), 19);

    let full: SuiteFile = toml::from_str(include_str!("../../suites/full_matrix.toml")).unwrap();
    assert_eq!(full.scenarios.len(), 23);

    let visual: SuiteFile =
        toml::from_str(include_str!("../../suites/visual_regression.toml")).unwrap();
    assert_eq!(visual.scenarios.len(), 3);

    let phone: SuiteFile = toml::from_str(include_str!("../../suites/phone_ui.toml")).unwrap();
    assert!(phone.runner.phone_layout);
    assert_eq!(phone.scenarios.len(), 1);

    let release_matrix: SuiteFile =
        toml::from_str(include_str!("../../suites/release_matrix_ui.toml")).unwrap();
    assert_eq!(release_matrix.scenarios.len(), 5);
    assert_eq!(
        release_matrix.scenarios[0].viewport.as_deref(),
        Some("700x600")
    );
}

#[test]
fn parses_per_scenario_ui_environment() {
    let suite: SuiteFile = toml::from_str(
        r#"
        [[scenario]]
        name = "matrix"
        path = "matrix.scn"
        viewport = "700x600"
        theme = "BlackAndWhite"
        language = "French"
        font_scale = 2.0
        reduced_motion = true
        release_channel = "Alpha"
        "#,
    )
    .unwrap();
    let scenario = &suite.scenarios[0];
    assert_eq!(scenario.viewport.as_deref(), Some("700x600"));
    assert_eq!(scenario.theme.as_deref(), Some("BlackAndWhite"));
    assert_eq!(scenario.language.as_deref(), Some("French"));
    assert_eq!(scenario.font_scale, Some(2.0));
    assert_eq!(scenario.reduced_motion, Some(true));
    assert_eq!(scenario.release_channel.as_deref(), Some("Alpha"));
}
