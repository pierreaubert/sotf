//! C/Rust bridge layer for Core Audio HAL driver callbacks
//!
//! This module provides the FFI interface between Core Audio's C-based HAL API
//! and our Rust implementation. It handles the conversion between C callbacks
//! and safe Rust abstractions.

use libc::{free, malloc};
use std::os::raw::c_void;
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};

use core_foundation::uuid::CFUUIDRef;
use coreaudio_sys::*;

use crate::{AudioDriverError, HALDriver, Result};

// Define Core Audio constants - using Apple's naming convention
#[allow(non_upper_case_globals)]
const kAudioHardwareNoError: OSStatus = 0;
#[allow(non_upper_case_globals)]
const kAudioHardwareUnspecifiedError: OSStatus = -1;
#[allow(non_upper_case_globals)]
const kAudioHardwareUnsupportedOperationError: OSStatus = -2;
#[allow(non_upper_case_globals)]
const kAudioHardwareBadParameterError: OSStatus = -3;
#[allow(non_upper_case_globals)]
const kAudioHardwareUnknownPropertyError: OSStatus = -4;

// Global driver instance - Core Audio expects a single driver instance
static DRIVER_INSTANCE: OnceLock<Arc<Mutex<HALDriver>>> = OnceLock::new();

/// Core Audio HAL driver interface structure
#[repr(C)]
pub struct AudioDriverPlugInInterface {
    pub interface: AudioServerPlugInDriverInterface,
}

/// Initialize the global driver instance
unsafe fn init_driver() -> Result<Arc<Mutex<HALDriver>>> {
    crate::init_logging();

    DRIVER_INSTANCE.get_or_init(|| match HALDriver::new() {
        Ok(driver) => {
            log::info!("HAL Driver initialized successfully");
            Arc::new(Mutex::new(driver))
        }
        Err(e) => {
            log::error!("Failed to initialize HAL driver: {}", e);
            panic!("Failed to initialize HAL driver: {}", e);
        }
    });

    DRIVER_INSTANCE
        .get()
        .ok_or_else(|| AudioDriverError::Device("Failed to initialize driver".to_string()).into())
        .map(|arc| arc.clone())
}

/// Get the global driver instance
unsafe fn get_driver() -> Result<Arc<Mutex<HALDriver>>> {
    DRIVER_INSTANCE
        .get()
        .ok_or_else(|| AudioDriverError::Device("Driver not initialized".to_string()).into())
        .map(|arc| arc.clone())
}

/// Entry point called when Core Audio loads the driver plugin
pub unsafe extern "C" fn audio_driver_plugin_open(
    driver_ref: *mut c_void, // Simplified host info
    driver: *mut *mut AudioServerPlugInDriverInterface,
) -> OSStatus {
    log::info!(
        "🚀 AudioDriverPlugInOpen called - driver_ref: {:p}, driver: {:p}",
        driver_ref,
        driver
    );

    // Initialize the driver
    log::info!("🔧 Initializing driver instance...");
    let driver_instance = match init_driver() {
        Ok(instance) => {
            log::info!("✅ Driver instance initialized successfully");
            instance
        }
        Err(e) => {
            log::error!("❌ Failed to initialize driver: {}", e);
            return kAudioHardwareUnspecifiedError;
        }
    };

    // Create the interface structure
    log::info!("📦 Allocating driver interface structure...");
    let interface = malloc(std::mem::size_of::<AudioDriverPlugInInterface>())
        as *mut AudioDriverPlugInInterface;
    if interface.is_null() {
        log::error!("❌ Failed to allocate driver interface");
        return kAudioHardwareUnspecifiedError;
    }
    log::info!("✅ Interface allocated at {:p}", interface);

    // Populate the interface with function pointers
    // Use transmute to convert between compatible but technically different function pointer types
    (*interface).interface = AudioServerPlugInDriverInterface {
        _reserved: ptr::null_mut(),
        QueryInterface: Some(std::mem::transmute(driver_query_interface as *const ())),
        AddRef: Some(std::mem::transmute(driver_add_ref as *const ())),
        Release: Some(std::mem::transmute(driver_release as *const ())),
        Initialize: Some(driver_initialize),
        CreateDevice: Some(driver_create_device),
        DestroyDevice: Some(driver_destroy_device),
        AddDeviceClient: Some(driver_add_device_client),
        RemoveDeviceClient: Some(driver_remove_device_client),
        PerformDeviceConfigurationChange: Some(driver_perform_device_configuration_change),
        AbortDeviceConfigurationChange: Some(driver_abort_device_configuration_change),
        HasProperty: Some(driver_has_property),
        IsPropertySettable: Some(driver_is_property_settable),
        GetPropertyDataSize: Some(driver_get_property_data_size),
        GetPropertyData: Some(driver_get_property_data),
        SetPropertyData: Some(driver_set_property_data),
        StartIO: Some(driver_start_io),
        StopIO: Some(driver_stop_io),
        GetZeroTimeStamp: Some(driver_get_zero_time_stamp),
        WillDoIOOperation: Some(driver_will_do_io_operation),
        BeginIOOperation: Some(driver_begin_io_operation),
        DoIOOperation: Some(driver_do_io_operation),
        EndIOOperation: Some(driver_end_io_operation),
    };

    // Store the host info for future reference
    if !driver_ref.is_null() {
        log::info!("📝 Setting host info...");
        if let Ok(mut driver_lock) = driver_instance.lock() {
            driver_lock.set_host_info(driver_ref);
            log::info!("✅ Host info set successfully");
        } else {
            log::error!("❌ Failed to lock driver for setting host info");
        }
    } else {
        log::warn!("⚠️  driver_ref is null, skipping host info");
    }

    *driver = &mut (*interface).interface;
    log::info!("📤 Returning driver interface pointer: {:p}", *driver);

    log::info!("✅ AudioDriverPlugInOpen completed successfully");
    kAudioHardwareNoError
}

/// Entry point called when Core Audio unloads the driver
pub unsafe extern "C" fn audio_driver_plugin_close(
    driver: *mut AudioServerPlugInDriverInterface,
) -> OSStatus {
    log::info!("AudioDriverPlugInClose called");

    if !driver.is_null() {
        // Calculate the offset to get back to our AudioDriverPlugInInterface
        let interface = (driver as *mut u8)
            .offset(-(std::mem::offset_of!(AudioDriverPlugInInterface, interface) as isize))
            as *mut AudioDriverPlugInInterface;

        free(interface as *mut c_void);
    }

    log::info!("AudioDriverPlugInClose completed");
    kAudioHardwareNoError
}

/// Factory function for creating driver instances
pub unsafe extern "C" fn audio_driver_plugin_factory(_uuid: CFUUIDRef) -> *mut c_void {
    log::info!("🏭 AudioDriverPlugInFactory called (not implemented, returning null)");

    // For now, we don't support the factory pattern - return null
    // Core Audio will use AudioDriverPlugInOpen instead
    ptr::null_mut()
}

// HAL Driver Interface Implementation Functions

unsafe extern "C" fn driver_query_interface(
    _driver: AudioServerPlugInDriverRef,
    _iid: REFIID,
    _interface: *mut LPVOID,
) -> HRESULT {
    log::debug!("driver_query_interface called");
    kAudioHardwareUnsupportedOperationError
}

unsafe extern "C" fn driver_add_ref(_driver: AudioServerPlugInDriverRef) -> ULONG {
    log::debug!("driver_add_ref called");
    1 // We don't use reference counting
}

unsafe extern "C" fn driver_release(_driver: AudioServerPlugInDriverRef) -> ULONG {
    log::debug!("driver_release called");
    1 // We don't use reference counting
}

unsafe extern "C" fn driver_initialize(
    _driver: AudioServerPlugInDriverRef,
    host: AudioServerPlugInHostRef,
) -> OSStatus {
    log::info!(
        "🔧 driver_initialize called - driver: {:p}, host: {:p}",
        _driver,
        host
    );

    let driver_instance = match get_driver() {
        Ok(instance) => instance,
        Err(e) => {
            log::error!("Failed to get driver instance: {}", e);
            return kAudioHardwareUnspecifiedError;
        }
    };

    let result = match driver_instance.lock() {
        Ok(mut driver_lock) => {
            log::info!("🔒 Driver locked, calling initialize...");
            match driver_lock.initialize(host as *mut _) {
                Ok(_) => {
                    log::info!("✅ Driver initialized successfully");
                    kAudioHardwareNoError
                }
                Err(e) => {
                    log::error!("❌ Driver initialization failed: {}", e);
                    kAudioHardwareUnspecifiedError
                }
            }
        }
        Err(e) => {
            log::error!("❌ Failed to lock driver: {}", e);
            kAudioHardwareUnspecifiedError
        }
    };
    log::info!("🏁 driver_initialize returning: {}", result);
    result
}

unsafe extern "C" fn driver_create_device(
    _driver: AudioServerPlugInDriverRef,
    _description: CFDictionaryRef,
    _client_info: *const AudioServerPlugInClientInfo,
    device_object_id: *mut AudioObjectID,
) -> OSStatus {
    log::info!(
        "🎤 driver_create_device called - description: {:p}, client_info: {:p}",
        _description,
        _client_info
    );

    let driver_instance = match get_driver() {
        Ok(instance) => instance,
        Err(_) => return kAudioHardwareUnspecifiedError,
    };

    let result = match driver_instance.lock() {
        Ok(mut driver_lock) => match driver_lock.create_device() {
            Ok(object_id) => {
                *device_object_id = object_id;
                log::info!("✅ Device created with ID: {}", object_id);
                kAudioHardwareNoError
            }
            Err(e) => {
                log::error!("❌ Failed to create device: {}", e);
                kAudioHardwareUnspecifiedError
            }
        },
        Err(e) => {
            log::error!("Failed to lock driver: {}", e);
            kAudioHardwareUnspecifiedError
        }
    };
    result
}

unsafe extern "C" fn driver_destroy_device(
    _driver: AudioServerPlugInDriverRef,
    device_object_id: AudioObjectID,
) -> OSStatus {
    log::info!(
        "driver_destroy_device called for device {}",
        device_object_id
    );

    let driver_instance = match get_driver() {
        Ok(instance) => instance,
        Err(_) => return kAudioHardwareUnspecifiedError,
    };

    let result = match driver_instance.lock() {
        Ok(mut driver_lock) => match driver_lock.destroy_device(device_object_id) {
            Ok(_) => {
                log::info!("Device {} destroyed", device_object_id);
                kAudioHardwareNoError
            }
            Err(e) => {
                log::error!("Failed to destroy device: {}", e);
                kAudioHardwareUnspecifiedError
            }
        },
        Err(e) => {
            log::error!("Failed to lock driver: {}", e);
            kAudioHardwareUnspecifiedError
        }
    };
    result
}

unsafe extern "C" fn driver_add_device_client(
    _driver: AudioServerPlugInDriverRef,
    device_object_id: AudioObjectID,
    _client_info: *const AudioServerPlugInClientInfo,
) -> OSStatus {
    log::info!(
        "driver_add_device_client called for device {}",
        device_object_id
    );
    kAudioHardwareNoError
}

unsafe extern "C" fn driver_remove_device_client(
    _driver: AudioServerPlugInDriverRef,
    device_object_id: AudioObjectID,
    _client_info: *const AudioServerPlugInClientInfo,
) -> OSStatus {
    log::info!(
        "driver_remove_device_client called for device {}",
        device_object_id
    );
    kAudioHardwareNoError
}

unsafe extern "C" fn driver_perform_device_configuration_change(
    _driver: AudioServerPlugInDriverRef,
    device_object_id: AudioObjectID,
    change_action: UInt64,
    _change_info: *mut c_void,
) -> OSStatus {
    log::info!(
        "driver_perform_device_configuration_change called for device {} with action {}",
        device_object_id,
        change_action
    );
    kAudioHardwareNoError
}

unsafe extern "C" fn driver_abort_device_configuration_change(
    _driver: AudioServerPlugInDriverRef,
    device_object_id: AudioObjectID,
    change_action: UInt64,
    _change_info: *mut c_void,
) -> OSStatus {
    log::info!(
        "driver_abort_device_configuration_change called for device {} with action {}",
        device_object_id,
        change_action
    );
    kAudioHardwareNoError
}

// Property handling functions
unsafe extern "C" fn driver_has_property(
    _driver: AudioServerPlugInDriverRef,
    object_id: AudioObjectID,
    _client_pid: pid_t,
    address: *const AudioObjectPropertyAddress,
) -> Boolean {
    if address.is_null() {
        log::warn!("⚠️  driver_has_property: address is null");
        return 0;
    }

    let addr = *address;
    log::debug!(
        "🔍 driver_has_property: object={} selector=0x{:08X} scope=0x{:08X} element={}",
        object_id,
        addr.mSelector,
        addr.mScope,
        addr.mElement
    );

    if let Ok(driver_instance) = get_driver() {
        if let Ok(driver_lock) = driver_instance.lock() {
            let has_prop = driver_lock.has_property(object_id, &addr);
            log::debug!("   → has_property result: {}", has_prop);
            if has_prop {
                return 1;
            }
        } else {
            log::error!("❌ Failed to lock driver in has_property");
        }
    } else {
        log::error!("❌ Failed to get driver in has_property");
    }
    0
}

unsafe extern "C" fn driver_is_property_settable(
    _driver: AudioServerPlugInDriverRef,
    object_id: AudioObjectID,
    _client_pid: pid_t,
    address: *const AudioObjectPropertyAddress,
    out_is_settable: *mut Boolean,
) -> OSStatus {
    if address.is_null() || out_is_settable.is_null() {
        return kAudioHardwareBadParameterError;
    }

    let addr = *address;
    log::debug!(
        "driver_is_property_settable called for object {} selector {}",
        object_id,
        addr.mSelector
    );

    let driver_instance = match get_driver() {
        Ok(instance) => instance,
        Err(_) => return kAudioHardwareUnspecifiedError,
    };

    let result = match driver_instance.lock() {
        Ok(driver_lock) => {
            *out_is_settable = if driver_lock.is_property_settable(object_id, &addr) {
                1
            } else {
                0
            };
            kAudioHardwareNoError
        }
        Err(_) => kAudioHardwareUnspecifiedError,
    };
    result
}

unsafe extern "C" fn driver_get_property_data_size(
    _driver: AudioServerPlugInDriverRef,
    object_id: AudioObjectID,
    _client_pid: pid_t,
    address: *const AudioObjectPropertyAddress,
    _qualifier_data_size: UInt32,
    _qualifier_data: *const c_void,
    out_data_size: *mut UInt32,
) -> OSStatus {
    if address.is_null() || out_data_size.is_null() {
        return kAudioHardwareBadParameterError;
    }

    let addr = *address;
    log::debug!(
        "driver_get_property_data_size called for object {} selector {}",
        object_id,
        addr.mSelector
    );

    let driver_instance = match get_driver() {
        Ok(instance) => instance,
        Err(_) => return kAudioHardwareUnspecifiedError,
    };

    let result = match driver_instance.lock() {
        Ok(driver_lock) => match driver_lock.get_property_data_size(object_id, &addr) {
            Ok(size) => {
                *out_data_size = size;
                kAudioHardwareNoError
            }
            Err(e) => {
                log::error!("get_property_data_size failed: {}", e);
                kAudioHardwareUnknownPropertyError
            }
        },
        Err(_) => kAudioHardwareUnspecifiedError,
    };
    result
}

unsafe extern "C" fn driver_get_property_data(
    _driver: AudioServerPlugInDriverRef,
    object_id: AudioObjectID,
    _client_pid: pid_t,
    address: *const AudioObjectPropertyAddress,
    _qualifier_data_size: UInt32,
    _qualifier_data: *const c_void,
    in_data_size: UInt32,
    out_data_size: *mut UInt32,
    out_data: *mut c_void,
) -> OSStatus {
    if address.is_null() || out_data.is_null() || out_data_size.is_null() {
        return kAudioHardwareBadParameterError;
    }

    let addr = *address;
    log::debug!(
        "driver_get_property_data called for object {} selector {}",
        object_id,
        addr.mSelector
    );

    let driver_instance = match get_driver() {
        Ok(instance) => instance,
        Err(_) => return kAudioHardwareUnspecifiedError,
    };

    let result = match driver_instance.lock() {
        Ok(driver_lock) => {
            let data_slice =
                std::slice::from_raw_parts_mut(out_data as *mut u8, in_data_size as usize);

            match driver_lock.get_property_data(object_id, &addr, data_slice) {
                Ok(bytes_written) => {
                    *out_data_size = bytes_written;
                    kAudioHardwareNoError
                }
                Err(e) => {
                    log::error!("get_property_data failed: {}", e);
                    kAudioHardwareUnknownPropertyError
                }
            }
        }
        Err(_) => kAudioHardwareUnspecifiedError,
    };
    result
}

unsafe extern "C" fn driver_set_property_data(
    _driver: AudioServerPlugInDriverRef,
    object_id: AudioObjectID,
    _client_pid: pid_t,
    address: *const AudioObjectPropertyAddress,
    _qualifier_data_size: UInt32,
    _qualifier_data: *const c_void,
    data_size: UInt32,
    data: *const c_void,
) -> OSStatus {
    if address.is_null() || data.is_null() {
        return kAudioHardwareBadParameterError;
    }

    let addr = *address;
    log::debug!(
        "driver_set_property_data called for object {} selector {}",
        object_id,
        addr.mSelector
    );

    let driver_instance = match get_driver() {
        Ok(instance) => instance,
        Err(_) => return kAudioHardwareUnspecifiedError,
    };

    let result = match driver_instance.lock() {
        Ok(mut driver_lock) => {
            let data_slice = std::slice::from_raw_parts(data as *const u8, data_size as usize);

            match driver_lock.set_property_data(object_id, &addr, data_slice) {
                Ok(_) => kAudioHardwareNoError,
                Err(e) => {
                    log::error!("set_property_data failed: {}", e);
                    kAudioHardwareUnknownPropertyError
                }
            }
        }
        Err(_) => kAudioHardwareUnspecifiedError,
    };
    result
}

// I/O Operation functions
unsafe extern "C" fn driver_start_io(
    _driver: AudioServerPlugInDriverRef,
    device_object_id: AudioObjectID,
    _client_id: UInt32,
) -> OSStatus {
    log::info!("driver_start_io called for device {}", device_object_id);

    let driver_instance = match get_driver() {
        Ok(instance) => instance,
        Err(_) => return kAudioHardwareUnspecifiedError,
    };

    let result = match driver_instance.lock() {
        Ok(mut driver_lock) => match driver_lock.start_io(device_object_id) {
            Ok(_) => {
                log::info!("Started I/O for device {}", device_object_id);
                kAudioHardwareNoError
            }
            Err(e) => {
                log::error!("Failed to start I/O: {}", e);
                kAudioHardwareUnspecifiedError
            }
        },
        Err(_) => kAudioHardwareUnspecifiedError,
    };
    result
}

unsafe extern "C" fn driver_stop_io(
    _driver: AudioServerPlugInDriverRef,
    device_object_id: AudioObjectID,
    _client_id: UInt32,
) -> OSStatus {
    log::info!("driver_stop_io called for device {}", device_object_id);

    let driver_instance = match get_driver() {
        Ok(instance) => instance,
        Err(_) => return kAudioHardwareUnspecifiedError,
    };

    let result = match driver_instance.lock() {
        Ok(mut driver_lock) => match driver_lock.stop_io(device_object_id) {
            Ok(_) => {
                log::info!("Stopped I/O for device {}", device_object_id);
                kAudioHardwareNoError
            }
            Err(e) => {
                log::error!("Failed to stop I/O: {}", e);
                kAudioHardwareUnspecifiedError
            }
        },
        Err(_) => kAudioHardwareUnspecifiedError,
    };
    result
}

unsafe extern "C" fn driver_get_zero_time_stamp(
    _driver: AudioServerPlugInDriverRef,
    _device_object_id: AudioObjectID,
    _client_id: UInt32,
    out_sample_time: *mut Float64,
    out_host_time: *mut UInt64,
    out_seed: *mut UInt64,
) -> OSStatus {
    if out_sample_time.is_null() || out_host_time.is_null() || out_seed.is_null() {
        return kAudioHardwareBadParameterError;
    }

    // For now, return current time
    *out_sample_time = 0.0;
    *out_host_time = mach_absolute_time();
    *out_seed = 1;

    kAudioHardwareNoError
}

unsafe extern "C" fn driver_will_do_io_operation(
    _driver: AudioServerPlugInDriverRef,
    _device_object_id: AudioObjectID,
    _client_id: UInt32,
    _operation_id: UInt32,
    out_will_do: *mut Boolean,
    out_will_do_in_place: *mut Boolean,
) -> OSStatus {
    // This is called before each I/O cycle to check if we'll handle the operation
    if !out_will_do.is_null() {
        *out_will_do = 1; // Yes, we will do this operation
    }
    if !out_will_do_in_place.is_null() {
        *out_will_do_in_place = 0; // No, we won't do it in-place
    }
    kAudioHardwareNoError
}

unsafe extern "C" fn driver_begin_io_operation(
    _driver: AudioServerPlugInDriverRef,
    _device_object_id: AudioObjectID,
    _client_id: UInt32,
    _operation_id: UInt32,
    _io_buffer_frame_size: UInt32,
    _io_cycle_info: *const AudioServerPlugInIOCycleInfo,
) -> OSStatus {
    // This is called at the beginning of each I/O cycle
    kAudioHardwareNoError
}

unsafe extern "C" fn driver_do_io_operation(
    _driver: AudioServerPlugInDriverRef,
    _device_object_id: AudioObjectID,
    _stream_object_id: AudioObjectID,
    _client_id: UInt32,
    _operation_id: UInt32,
    _io_buffer_frame_size: UInt32,
    _io_cycle_info: *const AudioServerPlugInIOCycleInfo,
    _io_main_buffer: *mut c_void,
    _io_secondary_buffer: *mut c_void,
) -> OSStatus {
    // This is the main I/O processing function
    // We'll implement the actual audio processing here

    let driver_instance = match get_driver() {
        Ok(instance) => instance,
        Err(_) => return kAudioHardwareUnspecifiedError,
    };

    let result = match driver_instance.lock() {
        Ok(_driver_lock) => {
            // TODO: Implement actual audio processing
            // For now, just return success
            kAudioHardwareNoError
        }
        Err(_) => kAudioHardwareUnspecifiedError,
    };
    result
}

unsafe extern "C" fn driver_end_io_operation(
    _driver: AudioServerPlugInDriverRef,
    _device_object_id: AudioObjectID,
    _client_id: UInt32,
    _operation_id: UInt32,
    _io_buffer_frame_size: UInt32,
    _io_cycle_info: *const AudioServerPlugInIOCycleInfo,
) -> OSStatus {
    // This is called at the end of each I/O cycle
    kAudioHardwareNoError
}

// Import mach_absolute_time function
extern "C" {
    fn mach_absolute_time() -> u64;
}
