//! MIDI connection and device management

use crate::config::MidiConfig;
use crate::device::{MidiDeviceInfo, MidiDeviceType};
use crate::error::{MidiError, Result};
use crate::message::MidiMessage;
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use parking_lot::Mutex;
use std::sync::Arc;

/// Callback type for MIDI input messages
pub type MidiCallback = Arc<dyn Fn(MidiMessage) + Send + Sync>;

/// Main MIDI manager for handling all MIDI operations
pub struct MidiManager {
    /// MIDI input client
    midi_input: Option<MidiInput>,

    /// MIDI output client
    midi_output: Option<MidiOutput>,

    /// Active input connection
    input_connection: Option<MidiInputConnection<()>>,

    /// Active output connection
    output_connection: Arc<Mutex<Option<MidiOutputConnection>>>,

    /// MIDI configuration
    config: MidiConfig,

    /// Callback for received MIDI messages
    _callback: Option<MidiCallback>,
}

impl MidiManager {
    /// Create a new MIDI manager
    pub fn new() -> Result<Self> {
        Ok(Self {
            midi_input: None,
            midi_output: None,
            input_connection: None,
            output_connection: Arc::new(Mutex::new(None)),
            config: MidiConfig::default(),
            _callback: None,
        })
    }

    /// Create a new MIDI manager with a configuration
    pub fn with_config(config: MidiConfig) -> Result<Self> {
        Ok(Self {
            midi_input: None,
            midi_output: None,
            input_connection: None,
            output_connection: Arc::new(Mutex::new(None)),
            config,
            _callback: None,
        })
    }

    /// List all available MIDI input devices
    pub fn list_input_devices(&mut self) -> Result<Vec<MidiDeviceInfo>> {
        let midi_in = MidiInput::new("SOTF MIDI Input")?;
        let ports = midi_in.ports();

        let devices = ports
            .iter()
            .enumerate()
            .map(|(index, port)| {
                let name = midi_in
                    .port_name(port)
                    .unwrap_or_else(|_| format!("Unknown Input {}", index));

                MidiDeviceInfo {
                    index,
                    name,
                    device_type: MidiDeviceType::Input,
                    manufacturer: None,
                    is_connected: false,
                }
            })
            .collect();

        // Keep the input client for later use
        self.midi_input = Some(midi_in);

        Ok(devices)
    }

    /// List all available MIDI output devices
    pub fn list_output_devices(&mut self) -> Result<Vec<MidiDeviceInfo>> {
        let midi_out = MidiOutput::new("SOTF MIDI Output")?;
        let ports = midi_out.ports();

        let devices = ports
            .iter()
            .enumerate()
            .map(|(index, port)| {
                let name = midi_out
                    .port_name(port)
                    .unwrap_or_else(|_| format!("Unknown Output {}", index));

                MidiDeviceInfo {
                    index,
                    name,
                    device_type: MidiDeviceType::Output,
                    manufacturer: None,
                    is_connected: false,
                }
            })
            .collect();

        // Keep the output client for later use
        self.midi_output = Some(midi_out);

        Ok(devices)
    }

    /// Connect to a MIDI input device by index
    pub fn connect_input<F>(&mut self, port_index: usize, callback: F) -> Result<()>
    where
        F: Fn(MidiMessage) + Send + Sync + 'static,
    {
        // Disconnect any existing connection
        self.disconnect_input();

        // Create input if not already created
        if self.midi_input.is_none() {
            self.midi_input = Some(MidiInput::new("SOTF MIDI Input")?);
        }

        let midi_in = self.midi_input.take().ok_or(MidiError::NotConnected)?;

        let ports = midi_in.ports();
        let port = ports
            .get(port_index)
            .ok_or(MidiError::InvalidDevice(port_index))?;

        let port_name = midi_in
            .port_name(port)
            .unwrap_or_else(|_| format!("Port {}", port_index));

        log::info!("Connecting to MIDI input: {}", port_name);

        // Connect with callback
        let connection = midi_in.connect(
            port,
            &format!("SOTF Input {}", port_name),
            move |_timestamp, message, _| {
                if let Ok(midi_msg) = MidiMessage::from_bytes(message) {
                    callback(midi_msg);
                }
            },
            (),
        )?;

        self.input_connection = Some(connection);

        Ok(())
    }

    /// Connect to a MIDI input device by name
    pub fn connect_input_by_name<F>(&mut self, name: &str, callback: F) -> Result<()>
    where
        F: Fn(MidiMessage) + Send + Sync + 'static,
    {
        let devices = self.list_input_devices()?;
        let device = devices
            .iter()
            .find(|d| d.name == name)
            .ok_or_else(|| MidiError::ConnectionError(format!("Device not found: {}", name)))?;

        self.connect_input(device.index, callback)
    }

    /// Disconnect from MIDI input
    pub fn disconnect_input(&mut self) {
        if let Some(connection) = self.input_connection.take() {
            connection.close();
            log::info!("Disconnected MIDI input");
        }
    }

    /// Connect to a MIDI output device by index
    pub fn connect_output(&mut self, port_index: usize) -> Result<()> {
        // Disconnect any existing connection
        self.disconnect_output();

        // Create output if not already created
        if self.midi_output.is_none() {
            self.midi_output = Some(MidiOutput::new("SOTF MIDI Output")?);
        }

        let midi_out = self.midi_output.take().ok_or(MidiError::NotConnected)?;

        let ports = midi_out.ports();
        let port = ports
            .get(port_index)
            .ok_or(MidiError::InvalidDevice(port_index))?;

        let port_name = midi_out
            .port_name(port)
            .unwrap_or_else(|_| format!("Port {}", port_index));

        log::info!("Connecting to MIDI output: {}", port_name);

        let connection = midi_out.connect(port, &format!("SOTF Output {}", port_name))?;

        *self.output_connection.lock() = Some(connection);

        Ok(())
    }

    /// Connect to a MIDI output device by name
    pub fn connect_output_by_name(&mut self, name: &str) -> Result<()> {
        let devices = self.list_output_devices()?;
        let device = devices
            .iter()
            .find(|d| d.name == name)
            .ok_or_else(|| MidiError::ConnectionError(format!("Device not found: {}", name)))?;

        self.connect_output(device.index)
    }

    /// Disconnect from MIDI output
    pub fn disconnect_output(&mut self) {
        let mut conn = self.output_connection.lock();
        if let Some(connection) = conn.take() {
            connection.close();
            log::info!("Disconnected MIDI output");
        }
    }

    /// Send a MIDI message
    pub fn send_message(&self, message: &MidiMessage) -> Result<()> {
        let mut conn = self.output_connection.lock();
        let connection = conn.as_mut().ok_or(MidiError::NotConnected)?;

        let bytes = message.to_bytes();
        connection
            .send(&bytes)
            .map_err(|e| MidiError::SendError(e.to_string()))?;

        log::debug!("Sent MIDI: {}", message.description());

        Ok(())
    }

    /// Send raw MIDI bytes
    pub fn send_raw(&self, bytes: &[u8]) -> Result<()> {
        let mut conn = self.output_connection.lock();
        let connection = conn.as_mut().ok_or(MidiError::NotConnected)?;

        connection
            .send(bytes)
            .map_err(|e| MidiError::SendError(e.to_string()))?;

        log::debug!("Sent raw MIDI: {:?}", bytes);

        Ok(())
    }

    /// Check if input is connected
    pub fn is_input_connected(&self) -> bool {
        self.input_connection.is_some()
    }

    /// Check if output is connected
    pub fn is_output_connected(&self) -> bool {
        self.output_connection.lock().is_some()
    }

    /// Get the current configuration
    pub fn config(&self) -> &MidiConfig {
        &self.config
    }

    /// Get mutable configuration
    pub fn config_mut(&mut self) -> &mut MidiConfig {
        &mut self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: MidiConfig) {
        self.config = config;
    }

    /// Send all initialization messages from the active profile
    pub fn send_init_messages(&self) -> Result<()> {
        if let Some(profile) = self.config.active_profile() {
            for msg in &profile.init_messages {
                self.send_raw(msg)?;
            }
            log::info!(
                "Sent {} initialization messages",
                profile.init_messages.len()
            );
        }
        Ok(())
    }
}

impl Drop for MidiManager {
    fn drop(&mut self) {
        self.disconnect_input();
        self.disconnect_output();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_creation() {
        let manager = MidiManager::new();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_device_enumeration() {
        let mut manager = MidiManager::new().unwrap();

        // Should not panic even if no devices are available
        let inputs = manager.list_input_devices();
        assert!(inputs.is_ok());

        let outputs = manager.list_output_devices();
        assert!(outputs.is_ok());
    }
}
