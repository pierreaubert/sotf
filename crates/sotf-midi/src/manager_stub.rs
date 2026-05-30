//! MIDI manager stub for platforms without MIDI support (tvOS).

use crate::config::MidiConfig;
use crate::device::{MidiDeviceChange, MidiDeviceInfo, MidiDeviceSnapshot};
use crate::error::Result;
use crate::message::MidiMessage;
use std::sync::Arc;

pub type MidiCallback = Arc<dyn Fn(MidiMessage) + Send + Sync>;

pub struct MidiManager {
    config: MidiConfig,
}

impl MidiManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            config: MidiConfig::default(),
        })
    }

    pub fn with_config(config: MidiConfig) -> Result<Self> {
        Ok(Self { config })
    }

    pub fn list_input_devices(&mut self) -> Result<Vec<MidiDeviceInfo>> {
        Ok(vec![])
    }

    pub fn list_output_devices(&mut self) -> Result<Vec<MidiDeviceInfo>> {
        Ok(vec![])
    }

    pub fn device_snapshot(&self) -> Result<MidiDeviceSnapshot> {
        Ok(MidiDeviceSnapshot::default())
    }

    pub fn poll_device_changes(
        &self,
        previous: &mut MidiDeviceSnapshot,
    ) -> Result<Vec<MidiDeviceChange>> {
        *previous = MidiDeviceSnapshot::default();
        Ok(Vec::new())
    }

    pub fn connect_input<F>(&mut self, _port_index: usize, _callback: F) -> Result<()>
    where
        F: Fn(MidiMessage) + Send + Sync + 'static,
    {
        Ok(())
    }

    pub fn connect_input_by_name<F>(&mut self, _name: &str, _callback: F) -> Result<()>
    where
        F: Fn(MidiMessage) + Send + Sync + 'static,
    {
        Ok(())
    }

    pub fn disconnect_input(&mut self) {}

    pub fn connect_output(&mut self, _port_index: usize) -> Result<()> {
        Ok(())
    }

    pub fn connect_output_by_name(&mut self, _name: &str) -> Result<()> {
        Ok(())
    }

    pub fn disconnect_output(&mut self) {}

    pub fn send_message(&self, _message: &MidiMessage) -> Result<()> {
        Ok(())
    }

    pub fn send_raw(&self, _bytes: &[u8]) -> Result<()> {
        Ok(())
    }

    pub fn send_clock_start(&self) -> Result<()> {
        Ok(())
    }

    pub fn send_clock_continue(&self) -> Result<()> {
        Ok(())
    }

    pub fn send_clock_stop(&self) -> Result<()> {
        Ok(())
    }

    pub fn send_clock_tick(&self) -> Result<()> {
        Ok(())
    }

    pub fn is_input_connected(&self) -> bool {
        false
    }

    pub fn is_output_connected(&self) -> bool {
        false
    }

    pub fn config(&self) -> &MidiConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut MidiConfig {
        &mut self.config
    }

    pub fn set_config(&mut self, config: MidiConfig) {
        self.config = config;
    }

    pub fn send_init_messages(&self) -> Result<()> {
        Ok(())
    }
}
