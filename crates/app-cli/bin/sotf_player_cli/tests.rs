use super::parse::parse_channel_mapping;
use super::parse::parse_filters;
use super::parse::parse_loudness_compensation;
use super::types::Cli;

#[test]
fn cli_definition_has_unique_argument_ids() {
    use clap::CommandFactory;

    Cli::command().debug_assert();
}

// -- parse_channel_mapping ------------------------------------------------

#[test]
fn parse_channel_mapping_rejects_zero_input_channel() {
    // Regression: previously `ch - 1` underflowed to usize::MAX for ch == 0.
    let err = parse_channel_mapping("0,1->1,2").expect_err("0-indexed input must fail");
    assert!(
        err.contains("Input channel index must be >= 1"),
        "unexpected error: {err}"
    );
}

#[test]
fn parse_channel_mapping_rejects_zero_output_channel() {
    let err = parse_channel_mapping("1,2->0,1").expect_err("0-indexed output must fail");
    assert!(err.contains(">= 1"), "unexpected error: {err}");
}

#[test]
fn parse_channel_mapping_happy_path_is_one_indexed() {
    let (inp, out, matrix, physical) = parse_channel_mapping("1,2->9,10").expect("valid mapping");
    assert_eq!(inp, vec![0, 1]);
    assert_eq!(out, vec![8, 9]);
    assert_eq!(matrix, vec![1.0, 0.0, 0.0, 1.0]);
    assert_eq!(physical, 10);
}

#[test]
fn parse_channel_mapping_supports_gap_underscore() {
    let (inp, out, _matrix, physical) = parse_channel_mapping("1,2->_,9,10").expect("gap mapping");
    assert_eq!(inp, vec![0, 1]);
    assert_eq!(out, vec![8, 9]);
    // The gap is a skipped physical index, but the device buffer still spans
    // the full hardware width.
    assert_eq!(physical, 10);
}

#[test]
fn parse_channel_mapping_rejects_unparseable_input() {
    assert!(parse_channel_mapping("x,1->1,2").is_err());
    assert!(parse_channel_mapping("1,2").is_err());
}

// -- channel-mapping round trips (parse -> sparse matrix plugin -> audio) ---

/// Drive the parsed mapping through the real downstream consumer
/// (`MatrixPlugin::with_sparse_mapping`) and check physical routing.
fn run_mapping_once(
    input_map: Vec<usize>,
    output_map: Vec<usize>,
    matrix: Vec<f32>,
    input_frame: Vec<f32>,
) -> Vec<f32> {
    use sotf_plugins::{MatrixPlugin, Plugin, ProcessContext};

    let mut plugin =
        MatrixPlugin::with_sparse_mapping(input_map, output_map, matrix).expect("sparse map");
    // `with_sparse_mapping` runs at a fixed 48 kHz smoother rate.
    let ctx = ProcessContext::new(48_000, 1);
    // Sentinel fill proves gap/unmapped channels are zeroed, not stale.
    let mut output = vec![7.0f32; plugin.output_channels()];
    let frames = plugin
        .process(&input_frame, &mut output, &ctx)
        .expect("process");
    assert_eq!(frames, 1);
    output
}

#[test]
fn parse_channel_mapping_plain_roundtrip_routes_to_hw_channels() {
    let (inp, out, matrix, physical) = parse_channel_mapping("1,2->9,10").expect("valid mapping");
    assert_eq!(physical, 10);
    assert_eq!(matrix.len(), inp.len() * out.len());

    let physical_in = inp.iter().max().map(|&v| v + 1).unwrap_or(0);
    let output = run_mapping_once(inp, out, matrix, vec![0.5, -0.25]);
    assert_eq!(output.len(), physical);
    assert_eq!(physical_in, 2);
    for (idx, sample) in output.iter().enumerate() {
        let expected = match idx {
            8 => 0.5,
            9 => -0.25,
            _ => 0.0,
        };
        assert!(
            (sample - expected).abs() < 1e-6,
            "physical ch {idx}: got {sample}, want {expected}"
        );
    }
}

#[test]
fn parse_channel_mapping_gap_position_survives_roundtrip() {
    // Gap at hardware position 4 (1-indexed): inputs must route around it,
    // and the gap channel itself must stay silent.
    let (inp, out, matrix, physical) =
        parse_channel_mapping("1,2,3,4,5->1,2,3,_,5,6").expect("gap mapping");
    assert_eq!(inp, vec![0, 1, 2, 3, 4]);
    assert_eq!(out, vec![0, 1, 2, 4, 5]);
    assert_eq!(physical, 6);
    assert_eq!(matrix.len(), inp.len() * out.len());

    let output = run_mapping_once(inp, out, matrix, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    assert_eq!(output.len(), physical);
    let expected = [1.0, 2.0, 3.0, 0.0, 4.0, 5.0];
    for (idx, (sample, want)) in output.iter().zip(expected.iter()).enumerate() {
        assert!(
            (sample - want).abs() < 1e-6,
            "physical ch {idx}: got {sample}, want {want}"
        );
    }
}

// -- parse_filters sample rate ------------------------------------------------

#[test]
fn parse_filters_uses_given_sample_rate() {
    let spec = vec!["1000:1.5:3.0".to_string()];
    let at_441 = parse_filters(&spec, 44_100.0).expect("44.1k filters");
    let at_48 = parse_filters(&spec, 48_000.0).expect("48k filters");
    assert_eq!(at_441.len(), 1);
    assert_eq!(at_48.len(), 1);
    assert_eq!(at_441[0].srate, 44_100.0);
    assert_eq!(at_48[0].srate, 48_000.0);
    // Golden check: the same spec baked at different rates must produce
    // different coefficients (a 48 kHz hard-code would make these identical).
    assert_ne!(format!("{:?}", at_441[0]), format!("{:?}", at_48[0]));
}

#[test]
fn parse_filters_rejects_bad_sample_rate() {
    let spec = vec!["1000:1.5:3.0".to_string()];
    assert!(parse_filters(&spec, 0.0).is_err());
    assert!(parse_filters(&spec, -44_100.0).is_err());
    assert!(parse_filters(&spec, f64::NAN).is_err());
    assert!(parse_filters(&spec, f64::INFINITY).is_err());
}

#[test]
fn parse_filters_rejects_frequency_above_nyquist() {
    // 20 kHz cannot be represented at a 32 kHz design rate.
    let specs = vec!["PK:20000:1.0:0.0".to_string()];
    let err = parse_filters(&specs, 32000.0).expect_err("above Nyquist must fail");
    assert!(err.contains("Nyquist"), "unexpected error: {err}");
}

// -- play help snapshot ------------------------------------------------------

#[test]
fn play_help_pins_all_plugin_flag_sets() {
    use clap::CommandFactory;

    Cli::command().debug_assert();

    let play = Cli::command()
        .get_subcommands()
        .find(|s| s.get_name() == "play")
        .expect("play subcommand")
        .clone();
    let mut cmd = play;
    let mut buf = Vec::new();
    cmd.write_long_help(&mut buf).expect("render play help");
    let help = String::from_utf8(buf).expect("help is UTF-8");

    // One enable flag per flattened plugin arg struct (24 groups): adding,
    // renaming, or dropping a plugin flag set fails here on purpose.
    const PLUGIN_ENABLE_FLAGS: &[&str] = &[
        "--upmixer",
        "--aae",
        "--binaural",
        "--gain",
        "--compressor",
        "--gate",
        "--limiter",
        "--expander",
        "--multiband-compressor",
        "--multiband-expander",
        "--xtc",
        "--denoiser",
        "--pnd",
        "--fletcher-munson",
        "--convolution",
        "--spectrum-analyzer",
        "--channel-mute-solo",
        "--ab-compare",
        "--band-split",
        "--band-merge",
        "--downmix",
        "--mono-to-stereo",
        "--crossfeed",
        "--matrix",
    ];
    assert_eq!(
        PLUGIN_ENABLE_FLAGS.len(),
        24,
        "expected one enable flag per plugin group"
    );
    for flag in PLUGIN_ENABLE_FLAGS {
        assert!(
            help.contains(flag),
            "play help is missing plugin flag set {flag}"
        );
    }
    for core in ["--filter", "--hwaudio-play", "--rack", "--lufs"] {
        assert!(help.contains(core), "play help is missing {core}");
    }
    // Global debug flags render on the top-level help, not the subcommand's.
    let mut top = Vec::new();
    Cli::command()
        .write_long_help(&mut top)
        .expect("render top-level help");
    let top_help = String::from_utf8(top).expect("help is UTF-8");
    assert!(
        top_help.contains("--show-urls"),
        "top-level help is missing the global --show-urls debug flag"
    );
}

// -- parse_loudness_compensation -----------------------------------------

#[test]
fn parse_loudness_compensation_accepts_two_values() {
    let res = parse_loudness_compensation(&[70.0, 3.0]).expect("2 values valid");
    assert!(res.is_some());
}

#[test]
fn parse_loudness_compensation_accepts_three_values() {
    let res = parse_loudness_compensation(&[70.0, 3.0, 4.0]).expect("3 values valid");
    assert!(res.is_some());
}

#[test]
fn parse_loudness_compensation_rejects_wrong_arity() {
    assert!(parse_loudness_compensation(&[]).is_err());
    assert!(parse_loudness_compensation(&[70.0]).is_err());
    assert!(parse_loudness_compensation(&[70.0, 3.0, 4.0, 5.0]).is_err());
}
