//! SOTF EQ Plugin Implementation
//!
//! A 4-band parametric equalizer using plinth-plugin with Slint UI.

use crate::editor::EqPluginEditor;
use crate::parameters::EqParameters;
use crate::processor::EqProcessor;
use plinth_plugin::{
    clap::{ClapPlugin, Feature as ClapFeature},
    vst3::{Subcategory as Vst3Subcategory, Vst3Plugin},
    Error, Event, Host, HostInfo, Plugin, ProcessorConfig,
};
use std::io::{Read, Write};
use std::rc::Rc;
use std::sync::Arc;

/// SOTF Parametric EQ Plugin
pub struct SotfEqPlugin {
    /// Plugin parameters
    parameters: Arc<EqParameters>,
}

impl Plugin for SotfEqPlugin {
    const NAME: &'static str = "SOTF EQ";
    const VENDOR: &'static str = "SOTF";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    type Processor = EqProcessor;
    type Editor = EqPluginEditor;
    type Parameters = EqParameters;

    fn new(_host_info: HostInfo) -> Self {
        Self {
            parameters: EqParameters::new(),
        }
    }

    fn with_parameters<T>(&self, f: impl FnMut(&Self::Parameters) -> T) -> T {
        let mut f = f;
        f(&self.parameters)
    }

    fn process_event(&mut self, _event: &Event) {
        // Parameters are handled automatically via the parameter system
    }

    fn create_processor(&mut self, config: ProcessorConfig) -> Self::Processor {
        EqProcessor::new(self.parameters.clone(), config.sample_rate)
    }

    fn create_editor(&mut self, host: Rc<dyn Host>) -> Self::Editor {
        EqPluginEditor::new(host, self.parameters.clone())
    }

    fn save_state(&self, _writer: &mut impl Write) -> Result<(), Error> {
        // State is saved via the parameter system automatically
        Ok(())
    }

    fn load_state(&mut self, _reader: &mut impl Read) -> Result<(), Error> {
        // State is loaded via the parameter system automatically
        Ok(())
    }
}

impl ClapPlugin for SotfEqPlugin {
    const CLAP_ID: &'static str = "org.spinorama.sotf-eq-slint";

    const FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Equalizer,
    ];
}

impl Vst3Plugin for SotfEqPlugin {
    // Convert "SOTFEqSlint001\0\0" to u128
    // The bytes are: S O T F E q S l i n t 0 0 1 \0 \0
    const CLASS_ID: u128 = u128::from_be_bytes(*b"SOTFEqSlint001\0\0");

    const SUBCATEGORIES: &'static [Vst3Subcategory] =
        &[Vst3Subcategory::Fx, Vst3Subcategory::Eq];
}
