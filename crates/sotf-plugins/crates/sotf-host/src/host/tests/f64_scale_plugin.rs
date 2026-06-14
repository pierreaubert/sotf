use super::super::daw_host::DawHost;
use super::super::graph_edge::GraphEdge;
use crate::plugin::Plugin;

struct F64ScalePlugin {
    pub(super) channels: usize,
    pub(super) factor: f64,
}

impl Plugin for F64ScalePlugin {
    fn info(&self) -> crate::plugin::PluginInfo {
        crate::plugin::PluginInfo::new("F64Scale", "0.1", "test")
    }
    fn input_channels(&self) -> usize {
        self.channels
    }
    fn output_channels(&self) -> usize {
        self.channels
    }
    fn parameters(&self) -> Vec<crate::parameters::Parameter> {
        vec![crate::parameters::Parameter::new_float(
            "factor", "Factor", 1.0, 0.0, 8.0,
        )]
    }
    fn set_parameter(
        &mut self,
        id: crate::parameters::ParameterId,
        value: crate::parameters::ParameterValue,
    ) -> Result<(), String> {
        if id.0 == "factor"
            && let crate::parameters::ParameterValue::Float(value) = value
        {
            self.factor = value as f64;
            return Ok(());
        }
        Err(format!("unknown parameter: {}", id.0))
    }
    fn get_parameter(
        &self,
        id: &crate::parameters::ParameterId,
    ) -> Option<crate::parameters::ParameterValue> {
        if id.0 == "factor" {
            Some(crate::parameters::ParameterValue::Float(self.factor as f32))
        } else {
            None
        }
    }
    fn process(
        &mut self,
        _: &[f32],
        _: &mut [f32],
        _: &crate::plugin::ProcessContext,
    ) -> Result<usize, String> {
        Err("f32 path should not be used".into())
    }
    fn process_f64(
        &mut self,
        input: &[f64],
        output: &mut [f64],
        ctx: &crate::plugin::ProcessContext,
    ) -> Result<usize, String> {
        for (o, &i) in output.iter_mut().zip(input.iter()) {
            *o = i * self.factor;
        }
        Ok(ctx.num_frames)
    }
    fn supports_f64(&self) -> bool {
        true
    }
}

#[test]
fn test_process_f64_uses_native_chain_when_supported() {
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(F64ScalePlugin {
        channels: 2,
        factor: 2.0,
    }))
    .unwrap();
    g.add_plugin(Box::new(F64ScalePlugin {
        channels: 2,
        factor: 3.0,
    }))
    .unwrap();

    let input = vec![0.25_f64, -0.5, 1.0, -1.0];
    let mut output = vec![0.0_f64; input.len()];
    let frames = g.process_f64(&input, &mut output).unwrap();

    assert_eq!(frames, 2);
    assert_eq!(output, vec![1.5, -3.0, 6.0, -6.0]);
}

#[test]
fn test_process_f64_sample_offset_parameter_event_splits_native_chain() {
    let mut g = DawHost::new(2, 48000);
    g.add_plugin(Box::new(F64ScalePlugin {
        channels: 2,
        factor: 1.0,
    }))
    .unwrap();
    g.build().unwrap();

    g.set_plugin_parameter_at(
        0,
        "factor",
        crate::parameters::ParameterValue::Float(0.5),
        2,
    )
    .unwrap();

    let input = vec![1.0_f64; 8];
    let mut output = vec![0.0_f64; 8];
    let frames = g.process_f64(&input, &mut output).unwrap();

    assert_eq!(frames, 4);
    assert_eq!(output, vec![1.0, 1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.5]);
}

#[test]
fn test_process_f64_uses_native_dag_when_supported() {
    let mut g = DawHost::new(1, 48000);
    let a = g
        .add_node(
            "a".into(),
            Box::new(F64ScalePlugin {
                channels: 1,
                factor: 2.0,
            }),
        )
        .unwrap();
    let b = g
        .add_node(
            "b".into(),
            Box::new(F64ScalePlugin {
                channels: 1,
                factor: 3.0,
            }),
        )
        .unwrap();
    let c = g
        .add_node(
            "c".into(),
            Box::new(F64ScalePlugin {
                channels: 1,
                factor: 5.0,
            }),
        )
        .unwrap();
    let d = g
        .add_node(
            "d".into(),
            Box::new(F64ScalePlugin {
                channels: 1,
                factor: 1.0,
            }),
        )
        .unwrap();
    g.add_edge(GraphEdge::new(a, b)).unwrap();
    g.add_edge(GraphEdge::new(a, c)).unwrap();
    g.add_edge(GraphEdge::new(b, d)).unwrap();
    g.add_edge(GraphEdge::new(c, d)).unwrap();
    g.build().unwrap();

    let input = vec![1.0_f64, 2.0, 3.0, 4.0];
    let mut output = vec![0.0_f64; input.len()];
    let frames = g.process_f64(&input, &mut output).unwrap();

    assert_eq!(frames, 4);
    assert_eq!(output, vec![16.0, 32.0, 48.0, 64.0]);
}
