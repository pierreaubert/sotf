//! Core Audio HAL driver implementation
//!
//! This module contains the main HAL driver logic that interfaces with
//! Core Audio to provide a virtual audio device.
//!
//! This is a simplified HAL driver that:
//! - Creates a virtual audio input/output device
//! - Forwards audio data to/from audio buffers
//! - Lets the audio player handle all processing and configuration

use crate::audio_buffer::AudioBuffer;
use crate::utils::AudioObjectIDGenerator;
use crate::{AudioDriverError, Result};
use coreaudio_sys::*;
use rtrb::{Consumer, Producer};
use std::collections::HashMap;
use std::os::raw::c_void;
use std::sync::Arc;

// Define Core Audio property selectors as hardcoded values
// These use Apple's camelCase naming convention to match Apple's official documentation
#[allow(non_upper_case_globals, dead_code)]
const kAudioObjectPropertyName: u32 = 1851878757; // 'name'
#[allow(non_upper_case_globals, dead_code)]
const kAudioObjectPropertyManufacturer: u32 = 1819107691; // 'lmak'
#[allow(non_upper_case_globals, dead_code)]
const kAudioObjectPropertyOwnedObjects: u32 = 1870098020; // 'ownd'
#[allow(non_upper_case_globals, dead_code)]
const kAudioObjectPropertyBaseClass: u32 = 1650680371; // 'bcls'
#[allow(non_upper_case_globals, dead_code)]
const kAudioObjectPropertyClass: u32 = 1668047219; // 'clas'
#[allow(non_upper_case_globals, dead_code)]
const kAudioObjectPropertyOwner: u32 = 1937008677; // 'stdv'

// Device properties
const kAudioDevicePropertyDeviceUID: u32 = 1969841184; // 'uid '
const kAudioDevicePropertyModelUID: u32 = 1836411236; // 'muid'
const kAudioDevicePropertyTransportType: u32 = 1953653102; // 'tran'
const kAudioDevicePropertyRelatedDevices: u32 = 1634755427; // 'akin'
const kAudioDevicePropertyClockDomain: u32 = 1668049764; // 'clkd'
const kAudioDevicePropertyDeviceIsAlive: u32 = 1818850926; // 'livn'
const kAudioDevicePropertyDeviceIsRunning: u32 = 1735354734; // 'goin'
const kAudioDevicePropertyDeviceCanBeDefaultDevice: u32 = 1684434036; // 'dflt'
const kAudioDevicePropertyDeviceCanBeDefaultSystemDevice: u32 = 1934849908; // 'sflt'
const kAudioDevicePropertyLatency: u32 = 1818393204; // 'ltnc'
const kAudioDevicePropertyNominalSampleRate: u32 = 1853059700; // 'nsrt'
const kAudioDevicePropertyAvailableNominalSampleRates: u32 = 1853059619; // 'nsr#'
const kAudioDevicePropertyIcon: u32 = 1768124270; // 'icon'
const kAudioDevicePropertyIsHidden: u32 = 1751737454; // 'hidn'
const kAudioDevicePropertyPreferredChannelsForStereo: u32 = 1684236338; // 'dch2'
const kAudioDevicePropertyPreferredChannelLayout: u32 = 1936879216; // 'srnd'

// Stream properties
const kAudioDevicePropertyStreams: u32 = 1937009779; // 'stm#'
const kAudioDevicePropertySafetyOffset: u32 = 1935894636; // 'saft'
const kAudioDevicePropertyBufferFrameSize: u32 = 1718056307; // 'fsiz'
const kAudioDevicePropertyBufferFrameSizeRange: u32 = 1718056306; // 'fsz#'
const kAudioDevicePropertyStreamConfiguration: u32 = 1935764588; // 'slay'
const kAudioDevicePropertyIOCycleUsage: u32 = 1852403571; // 'ncyc'
const kAudioDevicePropertyStreamFormat: u32 = 1936092009; // 'sfmt'
const kAudioDevicePropertyIOProcStreamUsage: u32 = 1937077093; // 'suse'

// Stream object properties
const kAudioStreamPropertyDirection: u32 = 1935960434; // 'sdir'
const kAudioStreamPropertyTerminalType: u32 = 1952807028; // 'term'
const kAudioStreamPropertyStartingChannel: u32 = 1935894638; // 'schn'
#[allow(dead_code)] // Same value as kAudioDevicePropertyLatency
const kAudioStreamPropertyLatency: u32 = 1818393204; // 'ltnc'
const kAudioStreamPropertyVirtualFormat: u32 = 1936092006; // 'sfma'
const kAudioStreamPropertyPhysicalFormat: u32 = 1885430386; // 'pft '
const kAudioStreamPropertyAvailableVirtualFormats: u32 = 1936092003; // 'sfm#'
const kAudioStreamPropertyAvailablePhysicalFormats: u32 = 1885430371; // 'pf'

// Object class IDs (FourCC codes)
#[allow(dead_code)]
const kAudioObjectClassID: u32 = 1633841004; // 'aobj'
#[allow(dead_code)]
const kAudioPlugInClassID: u32 = 1634757735; // 'aplg'
#[allow(dead_code)]
const kAudioDeviceClassID: u32 = 1633969526; // 'adev'
#[allow(dead_code)]
const kAudioStreamClassID: u32 = 1634956652; // 'astr'

// Transport types
const kAudioDeviceTransportTypeVirtual: u32 = 1986622068; // 'virt'

// Terminal types
#[allow(dead_code)]
const kAudioStreamTerminalTypeMicrophone: u32 = 1835623282; // 'micr'
#[allow(dead_code)]
const kAudioStreamTerminalTypeSpeaker: u32 = 1936747378; // 'spkr'

// Direction values
#[allow(dead_code)]
const kAudioDevicePropertyScopeInput: u32 = 1768976993; // 'inpt'
#[allow(dead_code)]
const kAudioDevicePropertyScopeOutput: u32 = 1869968496; // 'outp'

/// Type of audio object
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectType {
    Device,
    InputStream,
    OutputStream,
    Unknown,
}

/// The main HAL driver structure
pub struct HALDriver {
    /// Generator for unique object IDs
    id_generator: AudioObjectIDGenerator,

    /// Host interface provided by Core Audio
    host: Option<*mut AudioServerPlugInHostInterface>,

    /// Host info from Core Audio (simplified)
    host_info: Option<*mut c_void>,

    /// Map of device IDs to device objects
    devices: HashMap<AudioObjectID, VirtualAudioDevice>,

    /// Current sample rate
    sample_rate: f64,

    /// Buffer size in frames
    buffer_size: u32,

    /// Bidirectional audio buffer handles
    input_producer: Option<Producer<f32>>,
    output_consumer: Option<Consumer<f32>>,
}

// SAFETY: HALDriver contains raw pointers from Core Audio which are thread-safe.
// Core Audio guarantees that these pointers remain valid and can be used across threads.
unsafe impl Send for HALDriver {}
unsafe impl Sync for HALDriver {}

/// Virtual audio device representation
pub struct VirtualAudioDevice {
    /// Unique object ID for this device
    object_id: AudioObjectID,

    /// Device name
    name: String,

    /// Device UID (unique identifier)
    device_uid: String,

    /// Model UID
    model_uid: String,

    /// Manufacturer name
    manufacturer: String,

    /// Number of input channels
    input_channels: u32,

    /// Number of output channels
    output_channels: u32,

    /// Current sample rate
    sample_rate: f64,

    /// Whether I/O is currently running
    io_running: bool,

    /// Input stream ID
    input_stream_id: Option<AudioObjectID>,

    /// Output stream ID
    output_stream_id: Option<AudioObjectID>,
}

impl HALDriver {
    /// Create a new HAL driver instance
    pub fn new() -> Result<Self> {
        log::info!("🆕 Creating new HAL driver instance");

        let driver = Self {
            id_generator: AudioObjectIDGenerator::new(),
            host: None,
            host_info: None,
            devices: HashMap::new(),
            sample_rate: 48000.0,
            buffer_size: 512,
            input_producer: None,
            output_consumer: None,
        };
        log::info!(
            "✅ HAL driver instance created - sample_rate: {}, buffer_size: {}",
            driver.sample_rate,
            driver.buffer_size
        );
        Ok(driver)
    }

    /// Initialize the driver with the host interface
    pub fn initialize(&mut self, host: *mut AudioServerPlugInHostInterface) -> Result<()> {
        log::info!(
            "⚙️  Initializing HAL driver with host interface: {:p}",
            host
        );

        if host.is_null() {
            log::error!("❌ Host interface is null!");
            return Err(AudioDriverError::Device("Host interface is null".to_string()).into());
        }
        log::info!("✅ Host interface is valid");

        self.host = Some(host);
        log::info!("💾 Host interface stored");

        // Initialize audio buffer for bidirectional audio
        log::info!("🔊 Initializing audio buffer...");
        let channels = 2; // Stereo by default
        let capacity_ms = 500; // 500ms buffer

        // Initialize the global buffer first
        crate::audio_buffer::init_global_buffer(capacity_ms, self.sample_rate as u32, channels);

        // Then get access to it and take our handles
        if let Some(buffer) = crate::audio_buffer::get_global_buffer() {
            self.input_producer = buffer.take_input_producer();
            self.output_consumer = buffer.take_output_consumer();
            log::info!("✅ Audio buffer initialized and handles acquired");
        } else {
            log::error!("❌ Failed to get global audio buffer");
        }

        // Create the default virtual device
        log::info!("📦 Creating default virtual device...");
        let device_id = self.create_device()?;
        log::info!("✅ Created default virtual device with ID: {}", device_id);

        Ok(())
    }

    /// Set the host info
    pub fn set_host_info(&mut self, host_info: *mut c_void) {
        log::info!("📝 Setting host info: {:p}", host_info);
        self.host_info = Some(host_info);
        log::info!("✅ Host info stored");
    }

    /// Create a new virtual audio device
    pub fn create_device(&mut self) -> Result<AudioObjectID> {
        log::info!("🎵 Creating new virtual audio device...");
        let device_id = self.id_generator.next_id();
        let input_stream_id = self.id_generator.next_id();
        let output_stream_id = self.id_generator.next_id();
        log::info!(
            "🎯 Generated IDs - device: {}, input_stream: {}, output_stream: {}",
            device_id,
            input_stream_id,
            output_stream_id
        );

        let device = VirtualAudioDevice {
            object_id: device_id,
            name: "Audio HAL Driver".to_string(),
            device_uid: format!("com.audiohal.device.{}", device_id),
            model_uid: "com.audiohal.model.virtual".to_string(),
            manufacturer: crate::DRIVER_MANUFACTURER.to_string(),
            input_channels: 2,
            output_channels: 2,
            sample_rate: self.sample_rate,
            io_running: false,
            input_stream_id: Some(input_stream_id),
            output_stream_id: Some(output_stream_id),
        };

        log::info!("💻 Device configuration:");
        log::info!("   Name: {}", device.name);
        log::info!("   UID: {}", device.device_uid);
        log::info!("   Manufacturer: {}", device.manufacturer);
        log::info!(
            "   Channels: {}in/{}out",
            device.input_channels,
            device.output_channels
        );
        log::info!("   Sample Rate: {} Hz", device.sample_rate);

        self.devices.insert(device_id, device);
        log::info!(
            "✅ Device added to devices map (total devices: {})",
            self.devices.len()
        );

        log::info!(
            "✅ Created virtual device: ID={}, input_stream={}, output_stream={}",
            device_id,
            input_stream_id,
            output_stream_id
        );

        Ok(device_id)
    }

    /// Destroy a virtual audio device
    pub fn destroy_device(&mut self, device_id: AudioObjectID) -> Result<()> {
        if let Some(_device) = self.devices.remove(&device_id) {
            log::info!("Destroyed device {}", device_id);
            Ok(())
        } else {
            Err(AudioDriverError::Device(format!("Device not found: {}", device_id)).into())
        }
    }

    /// Determine the type of an audio object
    fn get_object_type(&self, object_id: AudioObjectID) -> ObjectType {
        // Check if it's a device
        if self.devices.contains_key(&object_id) {
            return ObjectType::Device;
        }

        // Check if it's a stream
        for device in self.devices.values() {
            if device.input_stream_id == Some(object_id) {
                return ObjectType::InputStream;
            }
            if device.output_stream_id == Some(object_id) {
                return ObjectType::OutputStream;
            }
        }

        ObjectType::Unknown
    }

    /// Check if the driver has a specific property
    #[allow(non_upper_case_globals)]
    pub fn has_property(
        &self,
        _object_id: AudioObjectID,
        address: &AudioObjectPropertyAddress,
    ) -> bool {
        let obj_type = self.get_object_type(_object_id);

        match address.mSelector {
            // Properties common to all objects
            kAudioObjectPropertyBaseClass |
            kAudioObjectPropertyClass |
            kAudioObjectPropertyOwner |
            kAudioObjectPropertyName |
            kAudioObjectPropertyManufacturer => true,

            // Device-specific properties
            kAudioDevicePropertyDeviceUID |
            kAudioDevicePropertyModelUID |
            kAudioDevicePropertyTransportType |
            kAudioDevicePropertyRelatedDevices |
            kAudioDevicePropertyClockDomain |
            kAudioDevicePropertyDeviceIsAlive |
            kAudioDevicePropertyDeviceIsRunning |
            kAudioDevicePropertyDeviceCanBeDefaultDevice |
            kAudioDevicePropertyDeviceCanBeDefaultSystemDevice |
            kAudioDevicePropertyLatency |
            kAudioDevicePropertyStreams |
            kAudioDevicePropertySafetyOffset |
            kAudioDevicePropertyNominalSampleRate |
            kAudioDevicePropertyAvailableNominalSampleRates |
            kAudioDevicePropertyIcon |
            kAudioDevicePropertyIsHidden |
            kAudioDevicePropertyPreferredChannelsForStereo |
            kAudioDevicePropertyPreferredChannelLayout |
            kAudioDevicePropertyBufferFrameSize |
            kAudioDevicePropertyBufferFrameSizeRange |
            kAudioDevicePropertyStreamConfiguration |
            kAudioDevicePropertyIOCycleUsage |
            kAudioDevicePropertyStreamFormat |
            kAudioDevicePropertyIOProcStreamUsage => {
                matches!(obj_type, ObjectType::Device)
            }

            // Stream-specific properties
            kAudioStreamPropertyDirection |
            kAudioStreamPropertyTerminalType |
            kAudioStreamPropertyStartingChannel |
            // kAudioStreamPropertyLatency - same value as kAudioDevicePropertyLatency, handled above
            kAudioStreamPropertyVirtualFormat |
            kAudioStreamPropertyPhysicalFormat |
            kAudioStreamPropertyAvailableVirtualFormats |
            kAudioStreamPropertyAvailablePhysicalFormats => {
                matches!(obj_type, ObjectType::InputStream | ObjectType::OutputStream)
            }

            _ => false,
        }
    }

    /// Check if a property is settable
    #[allow(non_upper_case_globals)]
    pub fn is_property_settable(
        &self,
        _object_id: AudioObjectID,
        address: &AudioObjectPropertyAddress,
    ) -> bool {
        matches!(
            address.mSelector,
            kAudioDevicePropertyNominalSampleRate | kAudioDevicePropertyBufferFrameSize
        )
    }

    /// Get the size of property data
    #[allow(non_upper_case_globals)]
    pub fn get_property_data_size(
        &self,
        _object_id: AudioObjectID,
        address: &AudioObjectPropertyAddress,
    ) -> Result<u32> {
        use crate::utils::cfstring_ref_size;

        match address.mSelector {
            // CFString properties
            kAudioObjectPropertyName
            | kAudioObjectPropertyManufacturer
            | kAudioDevicePropertyDeviceUID
            | kAudioDevicePropertyModelUID => Ok(cfstring_ref_size()),

            // U32 properties
            kAudioObjectPropertyClass
            | kAudioObjectPropertyBaseClass
            | kAudioObjectPropertyOwner
            | kAudioDevicePropertyTransportType
            | kAudioDevicePropertyClockDomain
            | kAudioDevicePropertyDeviceIsAlive
            | kAudioDevicePropertyDeviceIsRunning
            | kAudioDevicePropertyDeviceCanBeDefaultDevice
            | kAudioDevicePropertyDeviceCanBeDefaultSystemDevice
            | kAudioDevicePropertyIsHidden
            | kAudioDevicePropertyLatency
            | kAudioDevicePropertySafetyOffset
            | kAudioDevicePropertyBufferFrameSize
            | kAudioDevicePropertyIOCycleUsage
            | kAudioStreamPropertyDirection
            | kAudioStreamPropertyTerminalType
            | kAudioStreamPropertyStartingChannel => Ok(std::mem::size_of::<u32>() as u32),

            // F64 properties
            kAudioDevicePropertyNominalSampleRate => Ok(std::mem::size_of::<f64>() as u32),

            // Array properties
            kAudioDevicePropertyAvailableNominalSampleRates => {
                // Support 44.1, 48, and 96 kHz
                Ok((std::mem::size_of::<AudioValueRange>() * 3) as u32)
            }

            kAudioDevicePropertyStreams => {
                if let Some(_device) = self.devices.get(&_object_id) {
                    // Return both input and output stream IDs
                    Ok((std::mem::size_of::<AudioObjectID>() * 2) as u32)
                } else {
                    Ok(0)
                }
            }

            kAudioDevicePropertyBufferFrameSizeRange => {
                Ok(std::mem::size_of::<AudioValueRange>() as u32)
            }

            kAudioStreamPropertyVirtualFormat | kAudioStreamPropertyPhysicalFormat => {
                Ok(std::mem::size_of::<AudioStreamBasicDescription>() as u32)
            }

            kAudioStreamPropertyAvailableVirtualFormats
            | kAudioStreamPropertyAvailablePhysicalFormats => {
                // Support stereo 16/24/32-bit at various sample rates
                Ok(std::mem::size_of::<AudioStreamRangedDescription>() as u32 * 3)
            }

            _ => {
                log::warn!(
                    "Unhandled get_property_data_size selector: 0x{:08X}",
                    address.mSelector
                );
                Err(AudioDriverError::Device(format!(
                    "Unknown property selector: 0x{:08X}",
                    address.mSelector
                ))
                .into())
            }
        }
    }

    /// Get property data
    #[allow(non_upper_case_globals)]
    pub fn get_property_data(
        &self,
        _object_id: AudioObjectID,
        address: &AudioObjectPropertyAddress,
        buffer: &mut [u8],
    ) -> Result<u32> {
        use crate::utils::{copy_cfstring_to_buffer, copy_value_to_buffer};

        let obj_type = self.get_object_type(_object_id);

        match address.mSelector {
            // Object class
            kAudioObjectPropertyClass => {
                let class_id = match obj_type {
                    ObjectType::Device => kAudioDeviceClassID,
                    ObjectType::InputStream | ObjectType::OutputStream => kAudioStreamClassID,
                    ObjectType::Unknown => kAudioObjectClassID,
                };
                copy_value_to_buffer(&class_id, buffer)
            }

            kAudioObjectPropertyBaseClass => copy_value_to_buffer(&kAudioObjectClassID, buffer),

            kAudioObjectPropertyOwner => {
                // For devices, return 0 (system owned). For streams, return device ID
                let owner = match obj_type {
                    ObjectType::Device => 0u32,
                    ObjectType::InputStream | ObjectType::OutputStream => {
                        // Find the owning device
                        for device in self.devices.values() {
                            if device.input_stream_id == Some(_object_id)
                                || device.output_stream_id == Some(_object_id)
                            {
                                return copy_value_to_buffer(&device.object_id, buffer);
                            }
                        }
                        0u32
                    }
                    ObjectType::Unknown => 0u32,
                };
                copy_value_to_buffer(&owner, buffer)
            }

            // String properties
            kAudioObjectPropertyName => {
                let name = if let Some(device) = self.devices.get(&_object_id) {
                    &device.name
                } else {
                    "Audio HAL Driver"
                };
                copy_cfstring_to_buffer(name, buffer)
            }

            kAudioObjectPropertyManufacturer => {
                let manufacturer = if let Some(device) = self.devices.get(&_object_id) {
                    &device.manufacturer
                } else {
                    crate::DRIVER_MANUFACTURER
                };
                copy_cfstring_to_buffer(manufacturer, buffer)
            }

            kAudioDevicePropertyDeviceUID => {
                if let Some(device) = self.devices.get(&_object_id) {
                    copy_cfstring_to_buffer(&device.device_uid, buffer)
                } else {
                    copy_cfstring_to_buffer("com.audiohal.device.unknown", buffer)
                }
            }

            kAudioDevicePropertyModelUID => {
                if let Some(device) = self.devices.get(&_object_id) {
                    copy_cfstring_to_buffer(&device.model_uid, buffer)
                } else {
                    copy_cfstring_to_buffer("com.audiohal.model.virtual", buffer)
                }
            }

            // Device properties
            kAudioDevicePropertyTransportType => {
                copy_value_to_buffer(&kAudioDeviceTransportTypeVirtual, buffer)
            }

            kAudioDevicePropertyClockDomain => {
                let clock_domain = 0u32; // 0 means not part of any clock domain
                copy_value_to_buffer(&clock_domain, buffer)
            }

            kAudioDevicePropertyDeviceIsAlive => {
                let is_alive = 1u32; // Always alive
                copy_value_to_buffer(&is_alive, buffer)
            }

            kAudioDevicePropertyDeviceIsRunning => {
                let is_running = if let Some(device) = self.devices.get(&_object_id) {
                    if device.io_running { 1u32 } else { 0u32 }
                } else {
                    0u32
                };
                copy_value_to_buffer(&is_running, buffer)
            }

            kAudioDevicePropertyDeviceCanBeDefaultDevice
            | kAudioDevicePropertyDeviceCanBeDefaultSystemDevice => {
                let can_be_default = 1u32; // Yes, can be default
                copy_value_to_buffer(&can_be_default, buffer)
            }

            kAudioDevicePropertyIsHidden => {
                let is_hidden = 0u32; // Not hidden
                copy_value_to_buffer(&is_hidden, buffer)
            }

            kAudioDevicePropertyLatency | kAudioDevicePropertySafetyOffset => {
                let latency = 0u32; // Zero latency (adjust if needed)
                copy_value_to_buffer(&latency, buffer)
            }

            kAudioDevicePropertyBufferFrameSize => copy_value_to_buffer(&self.buffer_size, buffer),

            kAudioDevicePropertyBufferFrameSizeRange => {
                // Support buffer sizes from 64 to 4096 frames
                let range = AudioValueRange {
                    mMinimum: 64.0,
                    mMaximum: 4096.0,
                };
                copy_value_to_buffer(&range, buffer)
            }

            kAudioDevicePropertyNominalSampleRate => {
                copy_value_to_buffer(&self.sample_rate, buffer)
            }

            kAudioDevicePropertyAvailableNominalSampleRates => {
                // Support common sample rates
                let rates = [
                    AudioValueRange {
                        mMinimum: 44100.0,
                        mMaximum: 44100.0,
                    },
                    AudioValueRange {
                        mMinimum: 48000.0,
                        mMaximum: 48000.0,
                    },
                    AudioValueRange {
                        mMinimum: 96000.0,
                        mMaximum: 96000.0,
                    },
                ];
                let mut offset = 0;
                for rate in &rates {
                    let bytes = copy_value_to_buffer(rate, &mut buffer[offset..])?;
                    offset += bytes as usize;
                }
                Ok(offset as u32)
            }

            kAudioDevicePropertyStreams => {
                if let Some(device) = self.devices.get(&_object_id) {
                    let mut offset = 0;
                    if let Some(input_id) = device.input_stream_id {
                        let bytes = copy_value_to_buffer(&input_id, &mut buffer[offset..])?;
                        offset += bytes as usize;
                    }
                    if let Some(output_id) = device.output_stream_id {
                        let bytes = copy_value_to_buffer(&output_id, &mut buffer[offset..])?;
                        offset += bytes as usize;
                    }
                    Ok(offset as u32)
                } else {
                    Ok(0)
                }
            }

            _ => {
                log::warn!(
                    "Unhandled get_property_data selector: 0x{:08X} for object {}",
                    address.mSelector,
                    _object_id
                );
                Ok(0)
            }
        }
    }

    /// Set property data
    #[allow(non_upper_case_globals)]
    pub fn set_property_data(
        &mut self,
        _object_id: AudioObjectID,
        address: &AudioObjectPropertyAddress,
        buffer: &[u8],
    ) -> Result<()> {
        use crate::utils::read_value_from_buffer;

        match address.mSelector {
            kAudioDevicePropertyNominalSampleRate => {
                let new_rate: f64 = read_value_from_buffer(buffer)?;
                self.sample_rate = new_rate;
                log::info!("Set sample rate to {}", new_rate);
                Ok(())
            }
            kAudioDevicePropertyBufferFrameSize => {
                let new_size: u32 = read_value_from_buffer(buffer)?;
                self.buffer_size = new_size;
                log::info!("Set buffer size to {}", new_size);
                Ok(())
            }
            _ => {
                log::warn!(
                    "Unhandled set_property_data selector: {}",
                    address.mSelector
                );
                Ok(())
            }
        }
    }

    /// Start I/O for a device
    pub fn start_io(&mut self, device_id: AudioObjectID) -> Result<()> {
        if let Some(device) = self.devices.get_mut(&device_id) {
            device.io_running = true;
            log::info!("Started I/O for device {}", device_id);
            Ok(())
        } else {
            Err(AudioDriverError::Device(format!("Device not found: {}", device_id)).into())
        }
    }

    /// Stop I/O for a device
    pub fn stop_io(&mut self, device_id: AudioObjectID) -> Result<()> {
        if let Some(device) = self.devices.get_mut(&device_id) {
            device.io_running = false;
            log::info!("Stopped I/O for device {}", device_id);
            Ok(())
        } else {
            Err(AudioDriverError::Device(format!("Device not found: {}", device_id)).into())
        }
    }

    /// Process audio I/O - called by Core Audio
    ///
    /// This reads audio from macOS apps (input) and writes it to the input buffer,
    /// and reads from the output buffer to send back to macOS (loopback).
    pub fn process_io(
        &mut self,
        _device_id: AudioObjectID,
        input_data: Option<&[f32]>,
        output_data: Option<&mut [f32]>,
    ) -> Result<()> {
        // Write input audio (from macOS apps) to input buffer (our producer)
        if let Some(input) = input_data {
            if let Some(producer) = &mut self.input_producer {
                if let Ok(chunk) = producer.write_chunk_uninit(input.len()) {
                    chunk.fill_from_iter(input.iter().copied());
                } else {
                    // Buffer full
                    log::warn!("Input buffer full ({} samples)", input.len());
                }
            }
        }

        // Read from output buffer (loopback from audio player) to send back to macOS
        if let Some(output) = output_data {
            if let Some(consumer) = &mut self.output_consumer {
                if let Ok(chunk) = consumer.read_chunk(output.len()) {
                    let (s1, s2) = chunk.as_slices();
                    let len1 = s1.len();

                    if len1 > 0 {
                        output[..len1].copy_from_slice(s1);
                    }
                    if s2.len() > 0 {
                        output[len1..len1 + s2.len()].copy_from_slice(s2);
                    }
                    chunk.commit_all();
                } else {
                    // Not enough data - read what's available
                    let available = consumer.slots();
                    if available > 0 {
                        if let Ok(chunk) = consumer.read_chunk(available) {
                            let (s1, s2) = chunk.as_slices();
                            let len1 = s1.len();
                            output[..len1].copy_from_slice(s1);
                            if s2.len() > 0 {
                                output[len1..len1 + s2.len()].copy_from_slice(s2);
                            }
                            chunk.commit_all();

                            // Fill rest with zeros
                            if available < output.len() {
                                output[available..].fill(0.0);
                            }
                        }
                    } else {
                        // No data
                        output.fill(0.0);
                    }
                }
            } else {
                // No consumer available
                output.fill(0.0);
            }
        }

        Ok(())
    }

    /// Get the audio buffer for external access (e.g., from audio player)
    pub fn get_audio_buffer(&self) -> Option<Arc<AudioBuffer>> {
        crate::audio_buffer::get_global_buffer()
    }
}

impl Default for HALDriver {
    fn default() -> Self {
        Self::new().expect("Failed to create default HAL driver")
    }
}

impl VirtualAudioDevice {
    /// Check if I/O is currently running
    pub fn is_io_running(&self) -> bool {
        self.io_running
    }

    /// Get the device name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the object ID
    pub fn object_id(&self) -> AudioObjectID {
        self.object_id
    }
}
