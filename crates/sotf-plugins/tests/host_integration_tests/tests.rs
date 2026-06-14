use sotf_plugins::{
    DawHost, DenoiserPlugin, DownmixPlugin, GainPlugin, GraphEdge, InPlacePluginAdapter, Plugin,
    UpmixerPlugin, XtcPlugin, XtcPluginParams,
};

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
