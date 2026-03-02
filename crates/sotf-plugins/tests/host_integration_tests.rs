// Host integration tests that use plugin types from the facade crate.
// These were originally in host.rs but moved here because they depend on
// plugin implementations (GainPlugin, UpmixerPlugin, etc.) which are not
// available in the sotf-host crate.

use sotf_plugins::{
    DawHost, DenoiserPlugin, DownmixPlugin, GainPlugin, GraphEdge, InPlacePluginAdapter, Plugin,
    UpmixerPlugin, XtcPlugin, XtcPluginParams,
};

fn process_until_settled(host: &mut DawHost, input: &[f32], output: &mut [f32]) {
    let nf = input.len() / host.input_channels();
    for _ in 0..(100000 / nf + 1) {
        host.process(input, output).unwrap();
    }
}

#[test]
fn test_linear_chain() {
    let mut g = DawHost::new(2, 48000);
    let n1 = g
        .add_node(
            "g1".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    let n2 = g
        .add_node(
            "g2".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, -12.0412, 0.0,
            ))),
        )
        .unwrap();
    g.add_edge(GraphEdge::new(n1, n2)).unwrap();
    g.build().unwrap();
    let i = vec![1.0; 96];
    let mut o = vec![0.0; 96];
    process_until_settled(&mut g, &i, &mut o);
    for &s in &o {
        assert!((s - 0.25).abs() < 0.01);
    }
}

#[test]
fn test_stream_merge() {
    let mut g = DawHost::new(2, 48000);
    let mut gain1 = GainPlugin::with_smoothing(2, 0.0, 0.0);
    let mut gain2 = GainPlugin::with_smoothing(2, 0.0, 0.0);
    gain1.set_gain_linear(0.5);
    gain2.set_gain_linear(0.5);
    let n1 = g
        .add_node(
            "s".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    let n2 = g
        .add_node("g1".into(), Box::new(InPlacePluginAdapter::new(gain1)))
        .unwrap();
    let n3 = g
        .add_node("g2".into(), Box::new(InPlacePluginAdapter::new(gain2)))
        .unwrap();
    let n4 = g
        .add_node(
            "m".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    g.add_edge(GraphEdge::new(n1, n2)).unwrap();
    g.add_edge(GraphEdge::new(n1, n3)).unwrap();
    g.add_edge(GraphEdge::new(n2, n4)).unwrap();
    g.add_edge(GraphEdge::new(n3, n4)).unwrap();
    g.build().unwrap();
    let i = vec![1.0; 96];
    let mut o = vec![0.0; 96];
    process_until_settled(&mut g, &i, &mut o);
    for &s in &o {
        assert!((s - 1.0).abs() < 0.01);
    }
}

#[test]
fn test_pluginhost_api_linear_chain() {
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(InPlacePluginAdapter::new(
        GainPlugin::with_smoothing(2, 0.0, 0.0),
    )))
    .unwrap();
    g.add_plugin(Box::new(InPlacePluginAdapter::new(
        GainPlugin::with_smoothing(2, -12.0412, 0.0),
    )))
    .unwrap();
    let i = vec![1.0; 96];
    let mut o = vec![0.0; 96];
    process_until_settled(&mut g, &i, &mut o);
    for &s in &o {
        assert!((s - 0.25).abs() < 0.01);
    }
}

#[test]
fn test_pluginhost_api_remove_plugin() {
    let mut g = DawHost::new(2, 48000);
    let gain1 = GainPlugin::with_smoothing(2, 0.0, 0.0);
    let mut gain2 = GainPlugin::with_smoothing(2, 0.0, 0.0);
    let mut gain3 = GainPlugin::with_smoothing(2, 0.0, 0.0);
    gain2.set_gain_linear(0.5);
    gain3.set_gain_linear(0.5);
    g.add_plugin(Box::new(InPlacePluginAdapter::new(gain1)))
        .unwrap();
    g.add_plugin(Box::new(InPlacePluginAdapter::new(gain2)))
        .unwrap();
    g.add_plugin(Box::new(InPlacePluginAdapter::new(gain3)))
        .unwrap();
    let _ = g.remove_plugin(1).unwrap();
    let i = vec![1.0; 96];
    let mut o = vec![0.0; 96];
    process_until_settled(&mut g, &i, &mut o);
    for &s in &o {
        assert!((s - 0.5).abs() < 0.001);
    }
}

#[test]
fn test_cycle_detection() {
    let mut g = DawHost::new_default(48000);
    let n1 = g
        .add_node(
            "g1".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    let n2 = g
        .add_node(
            "g2".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    g.add_edge(GraphEdge::new(n1, n2)).unwrap();
    g.add_edge(GraphEdge::new(n2, n1)).unwrap();
    assert!(g.build().is_err());
}

#[test]
fn test_parallel_diamond() {
    let mut g = DawHost::new_default(48000);
    let n1 = g
        .add_node(
            "g1".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, -3.0103, 0.0,
            ))),
        )
        .unwrap();
    let n2 = g
        .add_node(
            "g2".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    let n3 = g
        .add_node(
            "g3".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    let n4 = g
        .add_node(
            "g4".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    g.add_edge(GraphEdge::new(n1, n2)).unwrap();
    g.add_edge(GraphEdge::new(n1, n3)).unwrap();
    g.add_edge(GraphEdge::new(n2, n4)).unwrap();
    g.add_edge(GraphEdge::new(n3, n4)).unwrap();
    let i = vec![1.0; 96];
    let mut o = vec![0.0; 96];
    process_until_settled(&mut g, &i, &mut o);
    for &s in &o {
        assert!((s - 1.414).abs() < 0.05);
    }
}

#[test]
fn test_parallel_processing_enabled() {
    let mut g = DawHost::new_default(48000);
    let n1 = g
        .add_node(
            "i".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    let mut pns = vec![];
    for i in 0..4 {
        let n = g
            .add_node(
                format!("p{}", i),
                Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                    2, 0.0, 0.0,
                ))),
            )
            .unwrap();
        g.add_edge(GraphEdge::new(n1, n)).unwrap();
        pns.push(n);
    }
    let on = g
        .add_node(
            "o".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    for &n in &pns {
        g.add_edge(GraphEdge::new(n, on)).unwrap();
    }
    let i = vec![1.0; 96];
    let mut o = vec![0.0; 96];
    process_until_settled(&mut g, &i, &mut o);
    for &s in &o {
        assert!((s - 4.0).abs() < 0.01);
    }
}

#[test]
fn test_parallel_processing_disabled() {
    let mut g = DawHost::new_default(48000);
    g.set_parallel_enabled(false);
    let n1 = g
        .add_node(
            "g1".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    let n2 = g
        .add_node(
            "g2".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    let n3 = g
        .add_node(
            "g3".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    g.add_edge(GraphEdge::new(n1, n2)).unwrap();
    g.add_edge(GraphEdge::new(n1, n3)).unwrap();
    let i = vec![1.0; 96];
    let mut o = vec![0.0; 96];
    process_until_settled(&mut g, &i, &mut o);
    for &s in &o {
        assert!((s - 2.0).abs() < 0.01);
    }
}

#[test]
fn test_latency_calculation() {
    let mut g = DawHost::new_default(48000);
    let n1 = g
        .add_node(
            "g1".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    let n2 = g
        .add_node(
            "g2".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    g.add_edge(GraphEdge::new(n1, n2)).unwrap();
    g.build().unwrap();
    assert_eq!(g.total_latency_samples(), 0);
}

#[test]
fn test_reset() {
    let mut g = DawHost::new_default(48000);
    let n1 = g
        .add_node(
            "g1".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    let n2 = g
        .add_node(
            "g2".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    g.add_edge(GraphEdge::new(n1, n2)).unwrap();
    g.build().unwrap();
    g.reset();
}

#[test]
fn test_pluginhost_api_channel_mismatch() {
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(InPlacePluginAdapter::new(
        GainPlugin::with_smoothing(2, 0.0, 0.0),
    )))
    .unwrap();
    assert!(
        g.add_plugin(Box::new(InPlacePluginAdapter::new(
            GainPlugin::with_smoothing(5, 0.0, 0.0)
        )))
        .is_err()
    );
}

#[test]
fn test_mixed_api_usage() {
    // Use the low-level node API directly instead of mixed add_plugin + add_node,
    // since chain_nodes is private and not accessible from integration tests.
    let mut g = DawHost::new(2, 48000);
    let ln = g
        .add_node(
            "half".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, -3.0103, 0.0,
            ))),
        )
        .unwrap();
    let ba = g
        .add_node(
            "ba".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    let bb = g
        .add_node(
            "bb".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    let m = g
        .add_node(
            "m".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    g.add_edge(GraphEdge::new(ln, ba)).unwrap();
    g.add_edge(GraphEdge::new(ln, bb)).unwrap();
    g.add_edge(GraphEdge::new(ba, m)).unwrap();
    g.add_edge(GraphEdge::new(bb, m)).unwrap();
    let i = vec![1.0; 96];
    let mut o = vec![0.0; 96];
    process_until_settled(&mut g, &i, &mut o);
    for &s in &o {
        assert!((s - 1.414).abs() < 0.05);
    }
}

#[test]
fn test_upmixer_frame_count_during_fillup() {
    let up = UpmixerPlugin::new(
        2048, "5.0", 1.0, 0.7, 0.5, 80.0, 0.5, 200.0, 0.0, 0.0, false, 0.0,
    );
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(up)).unwrap();
    let nf = 256;
    let i = vec![0.5; nf * 2];
    let mut o = vec![0.0; nf * 5];
    assert_eq!(g.process(&i, &mut o).unwrap(), nf);
    let mut got = false;
    for _ in 0..40 {
        o.fill(0.0);
        g.process(&i, &mut o).unwrap();
        if o.iter().any(|&s: &f32| s.abs() > 1e-10) {
            got = true;
            break;
        }
    }
    assert!(got);
}

#[test]
fn test_xtc_in_host() {
    let xtc = XtcPlugin::new(XtcPluginParams::default(), 48000).unwrap();
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(xtc)).unwrap();
    let nf = 256;
    let i = vec![0.5; nf * 2];
    let mut o = vec![0.0; nf * 2];
    assert_eq!(g.process(&i, &mut o).unwrap(), nf);
    let mut got = false;
    for _ in 0..40 {
        o.fill(0.0);
        g.process(&i, &mut o).unwrap();
        if o.iter().any(|&s: &f32| s.abs() > 1e-10) {
            got = true;
            break;
        }
    }
    assert!(got);
}

#[test]
fn test_denoiser_in_host() {
    let d = DenoiserPlugin::new(2, false);
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(InPlacePluginAdapter::new(d)))
        .unwrap();
    let nf = 256;
    let i = vec![0.5; nf * 2];
    let mut o = vec![0.0; nf * 2];
    assert_eq!(g.process(&i, &mut o).unwrap(), nf);
}

#[test]
fn test_downmix_in_host() {
    let up = UpmixerPlugin::new(
        2048, "5.0", 1.0, 0.7, 0.5, 80.0, 0.5, 200.0, 0.0, 0.0, false, 0.0,
    );
    let dm = DownmixPlugin::new(up.output_channels());
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(up)).unwrap();
    g.add_plugin(Box::new(dm)).unwrap();
    let nf = 256;
    let i = vec![0.5; nf * 2];
    let mut o = vec![0.0; nf * 2];
    assert_eq!(g.process(&i, &mut o).unwrap(), nf);
    let mut got = false;
    for _ in 0..40 {
        o.fill(0.0);
        g.process(&i, &mut o).unwrap();
        if o.iter().any(|&s: &f32| s.abs() > 1e-10) {
            got = true;
            break;
        }
    }
    assert!(got);
}

#[test]
fn test_parallel_variable_frame_with_gain() {
    let mut g = DawHost::new(2, 48000);
    let input_node = g
        .add_node(
            "input".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    let n2 = g
        .add_node(
            "g2".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    let n3 = g
        .add_node(
            "g3".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    let output_node = g
        .add_node(
            "out".into(),
            Box::new(InPlacePluginAdapter::new(GainPlugin::with_smoothing(
                2, 0.0, 0.0,
            ))),
        )
        .unwrap();
    g.add_edge(GraphEdge::new(input_node, n2)).unwrap();
    g.add_edge(GraphEdge::new(input_node, n3)).unwrap();
    g.add_edge(GraphEdge::new(n2, output_node)).unwrap();
    g.add_edge(GraphEdge::new(n3, output_node)).unwrap();
    g.build().unwrap();

    let nf = 256;
    let i = vec![1.0; nf * 2];
    let mut o = vec![0.0; nf * 2];
    process_until_settled(&mut g, &i, &mut o);
    for &s in &o {
        assert!((s - 2.0).abs() < 0.01);
    }
}
