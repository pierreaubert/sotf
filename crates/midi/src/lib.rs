//! MIDI device management and control for SOTF audio players
//!
//! This crate provides functionality for:
//! - Enumerating available MIDI input/output devices
//! - Managing MIDI connections (input and output)
//! - Sending and receiving MIDI messages
//! - Configuring and persisting device profiles
//! - Pre-configured profiles for popular audio hardware
//!
//! # Examples
//!
//! ## Basic MIDI I/O
//!
//! ```no_run
//! use sotf_audio_player_midi::{MidiManager, MidiMessage};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a MIDI manager
//! let mut manager = MidiManager::new()?;
//!
//! // List available devices
//! let input_devices = manager.list_input_devices()?;
//! let output_devices = manager.list_output_devices()?;
//!
//! // Connect to devices
//! if !input_devices.is_empty() {
//!     manager.connect_input(0, |message| {
//!         println!("Received MIDI: {:?}", message);
//!     })?;
//! }
//!
//! if !output_devices.is_empty() {
//!     manager.connect_output(0)?;
//!
//!     // Send a note on message
//!     manager.send_message(&MidiMessage::NoteOn {
//!         channel: 0,
//!         note: 60,
//!         velocity: 100,
//!     })?;
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Using Device Profiles
//!
//! ```no_run
//! use sotf_audio_player_midi::{MidiManager, profiles::{TotalMixControl, TotalMixRow}};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut manager = MidiManager::new()?;
//! manager.connect_output(0)?;
//!
//! // Control RME TotalMix FX
//! let totalmix = TotalMixControl::new(&mut manager)?;
//! totalmix.set_main_volume(100)?;
//! totalmix.set_fader(TotalMixRow::Output, 0, 0, 95)?;
//! # Ok(())
//! # }
//! ```

pub mod auto_map;
pub mod config;
pub mod device;
pub mod error;
pub mod layout;
pub mod layouts;
pub mod manager;
pub mod mapping;
pub mod mapping_engine;
pub mod message;
pub mod profiles;
pub mod templates;

pub use config::{DeviceConfig, DeviceProfile, MidiConfig};
pub use device::{MidiDevice, MidiDeviceInfo};
pub use error::{MidiError, Result};
pub use layout::{ControllerLayout, MidiControlId, PhysicalControl, PhysicalControlKind};
pub use manager::MidiManager;
pub use mapping::{ControlBinding, MidiMapping, MidiOverlay, ValueScaling};
pub use mapping_engine::{MappingAction, MidiMappingEngine};
pub use message::MidiMessage;
pub use templates::TemplateRegistry;
