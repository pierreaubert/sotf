use crate::error::Error;
use std::ptr::{null, NonNull};
use std :: { mem } ;
use libc::pid_t;
use objc2_core_audio :: { kAudioDevicePropertyDeviceNameCFString , kAudioDevicePropertyHogMode , kAudioDevicePropertyScopeOutput , kAudioDevicePropertyStreamConfiguration , kAudioHardwareNoError , kAudioHardwarePropertyDefaultInputDevice , kAudioHardwarePropertyDefaultOutputDevice , kAudioHardwarePropertyDevices , kAudioObjectPropertyElementMaster , kAudioObjectPropertyElementWildcard , kAudioObjectPropertyScopeGlobal , kAudioObjectPropertyScopeInput , kAudioObjectPropertyScopeOutput , kAudioObjectSystemObject , kAudioStreamPropertyAvailablePhysicalFormats , kAudioStreamPropertyPhysicalFormat , AudioDeviceID , AudioObjectGetPropertyData , AudioObjectGetPropertyDataSize , AudioObjectID , AudioObjectPropertyAddress , AudioObjectPropertyScope , AudioStreamRangedDescription } ;
use objc2_core_audio_types :: { AudioBufferList , AudioStreamBasicDescription } ;
use objc2_core_foundation::CFString;
use crate::audio_unit::audio_format::{AudioFormat, LinearPcmFlags};
use crate::audio_unit::sample_format::SampleFormat;
use crate::audio_unit::stream_format::StreamFormat;
use crate :: audio_unit :: { Scope } ;

/// Helper function to get the device id of the default input or output device.
pub fn get_default_device_id(input: bool) -> Option<AudioDeviceID> {
    let selector = if input {
        kAudioHardwarePropertyDefaultInputDevice
    } else {
        kAudioHardwarePropertyDefaultOutputDevice
    };
    let property_address = AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };

    let mut audio_device_id: AudioDeviceID = 0;
    let data_size = mem::size_of::<AudioDeviceID>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject as AudioObjectID,
            NonNull::from(&property_address),
            0,
            null(),
            NonNull::from(&data_size),
            NonNull::from(&mut audio_device_id).cast(),
        )
    };
    if status != kAudioHardwareNoError as i32 {
        return None;
    }

    Some(audio_device_id)
}

/// Find the device id for a device name.
/// Set `input` to `true` to find a playback device, or `false` for a capture device.
pub fn get_device_id_from_name(name: &str, input: bool) -> Option<AudioDeviceID> {
    let scope = match input {
        false => Scope::Output,
        true => Scope::Input,
    };
    if let Ok(all_ids) = get_audio_device_ids() {
        return all_ids
            .iter()
            .find(|id| {
                get_device_name(**id).unwrap_or_default() == name
                    && get_audio_device_supports_scope(**id, scope).unwrap_or_default()
            })
            .copied();
    }
    None
}

/// List all audio device ids on the system.
pub fn get_audio_device_ids_for_scope(scope: Scope) -> Result<Vec<AudioDeviceID>, Error> {
    let dev_scope = match scope {
        Scope::Input => kAudioObjectPropertyScopeInput,
        Scope::Output => kAudioObjectPropertyScopeOutput,
        _ => kAudioObjectPropertyScopeGlobal,
    };
    let property_address = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDevices,
        mScope: dev_scope,
        mElement: kAudioObjectPropertyElementMaster,
    };

    macro_rules! try_status_or_return {
        ($status:expr) => {
            if $status != kAudioHardwareNoError as i32 {
                return Err(Error::Unknown($status));
            }
        };
    }

    let mut data_size = 0u32;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(
            kAudioObjectSystemObject as AudioObjectID,
            NonNull::from(&property_address),
            0,
            null(),
            NonNull::from(&mut data_size),
        )
    };
    try_status_or_return!(status);

    let device_count = data_size / mem::size_of::<AudioDeviceID>() as u32;
    let mut audio_devices = vec![];
    audio_devices.reserve_exact(device_count as usize);
    unsafe { audio_devices.set_len(device_count as usize) };

    let status = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject as AudioObjectID,
            NonNull::from(&property_address),
            0,
            null(),
            NonNull::from(&data_size),
            NonNull::new(audio_devices.as_mut_ptr()).unwrap().cast(),
        )
    };
    try_status_or_return!(status);
    Ok(audio_devices)
}

pub fn get_audio_device_ids() -> Result<Vec<AudioDeviceID>, Error> {
    get_audio_device_ids_for_scope(Scope::Global)
}

#[test]
fn test_get_audio_device_ids() {
    let _ = get_audio_device_ids().expect("Failed to get audio device ids");
}

#[test]
fn test_get_audio_device_ids_for_scope() {
    for scope in &[
        Scope::Global,
        Scope::Input,
        Scope::Output,
        Scope::Group,
        Scope::Part,
        Scope::Note,
        Scope::Layer,
        Scope::LayerItem,
    ] {
        let _ = get_audio_device_ids_for_scope(*scope).expect("Failed to get audio device ids");
    }
}

/// does this device support input / ouptut?
pub fn get_audio_device_supports_scope(devid: AudioDeviceID, scope: Scope) -> Result<bool, Error> {
    let dev_scope: AudioObjectPropertyScope = match scope {
        Scope::Input => kAudioObjectPropertyScopeInput,
        Scope::Output => kAudioObjectPropertyScopeOutput,
        _ => kAudioObjectPropertyScopeGlobal,
    };
    let property_address = AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyStreamConfiguration,
        mScope: dev_scope,
        mElement: kAudioObjectPropertyElementWildcard,
    };

    macro_rules! try_status_or_return {
        ($status:expr) => {
            if $status != kAudioHardwareNoError as i32 {
                return Err(Error::Unknown($status));
            }
        };
    }

    let mut data_size = 0u32;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(
            devid,
            NonNull::from(&property_address),
            0,
            null(),
            NonNull::from(&mut data_size),
        )
    };
    try_status_or_return!(status);

    let mut bfrs: Vec<u8> = Vec::with_capacity(data_size as usize);
    let buffers = bfrs.as_mut_ptr() as *mut AudioBufferList;
    unsafe {
        let status = AudioObjectGetPropertyData(
            devid,
            NonNull::from(&property_address),
            0,
            null(),
            NonNull::from(&data_size),
            NonNull::new(buffers).unwrap().cast(),
        );
        if status != kAudioHardwareNoError as i32 {
            return Err(Error::Unknown(status));
        }

        for i in 0..(*buffers).mNumberBuffers {
            let buf = (*buffers).mBuffers[i as usize];
            if buf.mNumberChannels > 0 {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Get the device name for a device id.
pub fn get_device_name(device_id: AudioDeviceID) -> Result<String, Error> {
    let property_address = AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyDeviceNameCFString,
        mScope: kAudioDevicePropertyScopeOutput,
        mElement: kAudioObjectPropertyElementMaster,
    };

    macro_rules! try_status_or_return {
        ($status:expr) => {
            if $status != kAudioHardwareNoError as i32 {
                return Err(Error::Unknown($status));
            }
        };
    }

    let mut device_name: *const CFString = null();
    let data_size = mem::size_of::<*const CFString>() as u32;
    unsafe {
        let status = AudioObjectGetPropertyData(
            device_id,
            NonNull::from(&property_address),
            0,
            null(),
            NonNull::from(&data_size),
            NonNull::from(&mut device_name).cast(),
        );
        try_status_or_return!(status);

        Ok((&*device_name).to_string())
    }
}

/// Find the closest match of the physical formats to the provided `StreamFormat`.
/// This function will pick the first format it finds that supports the provided sample format, rate and number of channels.
/// The provided format flags in the `StreamFormat` are ignored.
pub fn find_matching_physical_format(
    device_id: AudioDeviceID,
    stream_format: StreamFormat,
) -> Option<AudioStreamBasicDescription> {
    if let Ok(all_formats) = get_supported_physical_stream_formats(device_id) {
        let requested_samplerate = stream_format.sample_rate as usize;
        let requested_bits = stream_format.sample_format.size_in_bits();
        let requested_float = stream_format.sample_format == SampleFormat::F32;
        let requested_channels = stream_format.channels;
        for fmt in all_formats {
            let min_rate = fmt.mSampleRateRange.mMinimum as usize;
            let max_rate = fmt.mSampleRateRange.mMaximum as usize;
            let rate = fmt.mFormat.mSampleRate as usize;
            let channels = fmt.mFormat.mChannelsPerFrame;
            if let Some(AudioFormat::LinearPCM(flags)) = AudioFormat::from_format_and_flag(
                fmt.mFormat.mFormatID,
                Some(fmt.mFormat.mFormatFlags),
            ) {
                let is_float = flags.contains(LinearPcmFlags::IS_FLOAT);
                let is_int = flags.contains(LinearPcmFlags::IS_SIGNED_INTEGER);
                if is_int && is_float {
                    // Probably never occurs, check just in case
                    continue;
                }
                if requested_float && !is_float {
                    // Wrong number type
                    continue;
                }
                if !requested_float && !is_int {
                    // Wrong number type
                    continue;
                }
                if requested_bits != fmt.mFormat.mBitsPerChannel {
                    // Wrong number of bits
                    continue;
                }
                if requested_channels > channels {
                    // Too few channels
                    continue;
                }
                if rate == requested_samplerate
                    || (requested_samplerate >= min_rate && requested_samplerate <= max_rate)
                {
                    return Some(fmt.mFormat);
                }
            }
        }
    }
    None
}

/// Get a vector with all supported physical formats as AudioBasicRangedDescriptions.
pub fn get_supported_physical_stream_formats(
    device_id: AudioDeviceID,
) -> Result<Vec<AudioStreamRangedDescription>, Error> {
    // Get available formats.
    let mut property_address = AudioObjectPropertyAddress {
        mSelector: kAudioStreamPropertyPhysicalFormat,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };
    let allformats = unsafe {
        property_address.mSelector = kAudioStreamPropertyAvailablePhysicalFormats;
        let mut data_size = 0u32;
        let status = AudioObjectGetPropertyDataSize(
            device_id,
            NonNull::from(&property_address),
            0,
            null(),
            NonNull::from(&mut data_size),
        );
        Error::from_os_status(status)?;
        let n_formats = data_size as usize / mem::size_of::<AudioStreamRangedDescription>();
        let mut formats: Vec<AudioStreamRangedDescription> = vec![];
        formats.reserve_exact(n_formats as usize);
        formats.set_len(n_formats);

        let status = AudioObjectGetPropertyData(
            device_id,
            NonNull::from(&property_address),
            0,
            null(),
            NonNull::from(&data_size),
            NonNull::new(formats.as_mut_ptr()).unwrap().cast(),
        );
        Error::from_os_status(status)?;
        formats
    };
    Ok(allformats)
}

/// Helper for hog mode (exclusive access).
/// Get the pid of the process that currently owns exclusive access to a device.
/// A pid value of -1 means no process owns exclusive access.
pub fn get_hogging_pid(device_id: AudioDeviceID) -> Result<pid_t, Error> {
    let property_address = AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyHogMode,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };
    let pid = unsafe {
        let mut temp_pid: pid_t = 0;
        let data_size = mem::size_of::<pid_t>() as u32;
        let status = AudioObjectGetPropertyData(
            device_id,
            NonNull::from(&property_address),
            0,
            null(),
            NonNull::from(&data_size),
            NonNull::from(&mut temp_pid).cast(),
        );
        Error::from_os_status(status)?;
        temp_pid
    };
    Ok(pid)
}

