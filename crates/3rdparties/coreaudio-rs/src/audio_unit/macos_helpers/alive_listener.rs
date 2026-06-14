use crate::error::Error;
use std::ptr::{null, NonNull};
use std::sync::atomic::{AtomicBool, Ordering};
use std :: { mem } ;
use objc2_core_audio :: { kAudioDevicePropertyDeviceIsAlive , kAudioObjectPropertyElementMaster , kAudioObjectPropertyScopeGlobal , AudioDeviceID , AudioObjectAddPropertyListener , AudioObjectGetPropertyData , AudioObjectID , AudioObjectPropertyAddress , AudioObjectPropertyListenerProc , AudioObjectRemovePropertyListener } ;
use crate::OSStatus;

/// An AliveListener is used to get notified when a device is disconnected.
pub struct AliveListener {
    pub(super) alive: Box<AtomicBool>,
    pub(super) device_id: AudioDeviceID,
    pub(super) property_address: AudioObjectPropertyAddress,
    pub(super) alive_listener: AudioObjectPropertyListenerProc,
}

impl Drop for AliveListener {
    fn drop(&mut self) {
        let _ = self.unregister();
    }
}

impl AliveListener {
    /// Create a new AliveListener for the given AudioDeviceID.
    /// The listener must be registered by calling `register()` in order to start receiving notifications.
    pub fn new(device_id: AudioDeviceID) -> AliveListener {
        // Add our listener callback.
        let property_address = AudioObjectPropertyAddress {
            mSelector: kAudioDevicePropertyDeviceIsAlive,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        };
        AliveListener {
            alive: Box::new(AtomicBool::new(true)),
            device_id,
            property_address,
            alive_listener: None,
        }
    }

    /// Register this listener to receive notifications.
    pub fn register(&mut self) -> Result<(), Error> {
        unsafe extern "C-unwind" fn alive_listener(
            device_id: AudioObjectID,
            _n_addresses: u32,
            _properties: NonNull<AudioObjectPropertyAddress>,
            self_ptr: *mut ::std::os::raw::c_void,
        ) -> OSStatus {
            let self_ptr: &mut AliveListener = &mut *(self_ptr as *mut AliveListener);
            let mut alive: u32 = 0;
            let data_size = mem::size_of::<u32>() as u32;
            let property_address = AudioObjectPropertyAddress {
                mSelector: kAudioDevicePropertyDeviceIsAlive,
                mScope: kAudioObjectPropertyScopeGlobal,
                mElement: kAudioObjectPropertyElementMaster,
            };
            let result = AudioObjectGetPropertyData(
                device_id,
                NonNull::from(&property_address),
                0,
                null(),
                NonNull::from(&data_size),
                NonNull::from(&mut alive).cast(),
            );
            self_ptr.alive.store(alive > 0, Ordering::SeqCst);
            result
        }

        // Add our listener callback.
        let status = unsafe {
            AudioObjectAddPropertyListener(
                self.device_id,
                NonNull::from(&self.property_address),
                Some(alive_listener),
                self as *const _ as *mut _,
            )
        };
        Error::from_os_status(status)?;
        self.alive_listener = Some(alive_listener);
        Ok(())
    }

    /// Unregister this listener to stop receiving notifications
    pub fn unregister(&mut self) -> Result<(), Error> {
        if self.alive_listener.is_some() {
            let status = unsafe {
                AudioObjectRemovePropertyListener(
                    self.device_id,
                    NonNull::from(&self.property_address),
                    self.alive_listener,
                    self as *const _ as *mut _,
                )
            };
            Error::from_os_status(status)?;
            self.alive_listener = None;
        }
        Ok(())
    }

    /// Check if the device is still alive.
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }
}

