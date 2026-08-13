use super::native_backend::{NativeExternalPluginBackend, NativePluginMetadata};
use super::plugin_descriptor::PluginDescriptor;
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use objc2_audio_toolbox::{
    AURenderCallbackStruct, AudioComponent, AudioComponentCopyName, AudioComponentDescription,
    AudioComponentFindNext, AudioComponentGetDescription, AudioComponentGetVersion,
    AudioComponentInstanceDispose, AudioComponentInstanceNew, AudioUnit, AudioUnitGetParameter,
    AudioUnitGetProperty, AudioUnitGetPropertyInfo, AudioUnitInitialize, AudioUnitParameterInfo,
    AudioUnitParameterOptions, AudioUnitParameterUnit, AudioUnitRender, AudioUnitRenderActionFlags,
    AudioUnitSetParameter, AudioUnitSetProperty, AudioUnitUninitialize, MusicDeviceMIDIEvent,
    kAudioUnitProperty_ClassInfo, kAudioUnitProperty_Latency,
    kAudioUnitProperty_MaximumFramesPerSlice, kAudioUnitProperty_ParameterInfo,
    kAudioUnitProperty_ParameterList, kAudioUnitProperty_SetRenderCallback,
    kAudioUnitProperty_StreamFormat, kAudioUnitScope_Global, kAudioUnitScope_Input,
    kAudioUnitScope_Output, kAudioUnitType_Effect, kAudioUnitType_FormatConverter,
    kAudioUnitType_MusicEffect,
};
use objc2_core_audio_types::{
    AudioBuffer, AudioBufferList, AudioStreamBasicDescription, AudioTimeStamp, AudioTimeStampFlags,
    kAudioFormatFlagIsFloat, kAudioFormatFlagIsNonInterleaved, kAudioFormatFlagIsPacked,
    kAudioFormatLinearPCM,
};
use objc2_core_foundation::{
    CFData, CFError, CFPropertyList, CFPropertyListCreateData, CFPropertyListCreateWithData,
    CFPropertyListFormat, CFRetained, CFString,
};
use std::ffi::c_void;
use std::mem::{offset_of, size_of};
use std::ptr::{self, NonNull};
use std::slice;

const MAX_COMPONENTS: usize = 16_384;
const NO_ERR: i32 = 0;
const INVALID_PARAMETER: i32 = -50;

struct AudioUnitInputState {
    storage: *const f32,
    channels: usize,
    max_frames: usize,
}

#[derive(Clone, Copy)]
enum AudioUnitParameterKind {
    Float,
    Integer,
    Boolean,
}

struct AudioUnitParameterBinding {
    host_id: ParameterId,
    audio_unit_id: u32,
    kind: AudioUnitParameterKind,
}

unsafe extern "C-unwind" fn input_render_callback(
    state: NonNull<c_void>,
    _action_flags: NonNull<AudioUnitRenderActionFlags>,
    _time_stamp: NonNull<AudioTimeStamp>,
    _bus_number: u32,
    frames: u32,
    io_data: *mut AudioBufferList,
) -> i32 {
    if io_data.is_null() {
        return INVALID_PARAMETER;
    }
    // SAFETY: CoreAudio invokes the callback with the exact `inputProcRefCon`
    // retained by the backend and a writable buffer list for this render call.
    let state = unsafe { &*state.cast::<AudioUnitInputState>().as_ptr() };
    let frames = frames as usize;
    if frames > state.max_frames {
        return INVALID_PARAMETER;
    }
    // SAFETY: `mNumberBuffers` describes the variable-length trailing array
    // allocated and owned by CoreAudio for the callback duration.
    let list = unsafe { &mut *io_data };
    if list.mNumberBuffers as usize != state.channels {
        return INVALID_PARAMETER;
    }
    let buffers = unsafe {
        slice::from_raw_parts_mut(list.mBuffers.as_mut_ptr(), list.mNumberBuffers as usize)
    };
    let bytes = frames.saturating_mul(size_of::<f32>());
    for (channel, buffer) in buffers.iter_mut().enumerate() {
        if buffer.mNumberChannels != 1
            || buffer.mData.is_null()
            || (buffer.mDataByteSize as usize) < bytes
        {
            return INVALID_PARAMETER;
        }
        // SAFETY: The source channel is within fixed planar storage and the AU
        // provided at least `bytes` writable bytes for this channel.
        unsafe {
            ptr::copy_nonoverlapping(
                state.storage.add(channel * state.max_frames),
                buffer.mData.cast::<f32>(),
                frames,
            );
        }
        buffer.mDataByteSize = bytes as u32;
    }
    NO_ERR
}

struct AudioBufferListStorage {
    words: Box<[usize]>,
    channels: usize,
}

impl AudioBufferListStorage {
    fn new(
        channels: usize,
        max_block_frames: usize,
        planar_storage: &mut [f32],
    ) -> Result<Self, String> {
        if channels == 0 {
            return Err("AudioUnit output must contain at least one channel".to_string());
        }
        let bytes = offset_of!(AudioBufferList, mBuffers)
            .checked_add(channels.saturating_mul(size_of::<AudioBuffer>()))
            .ok_or_else(|| "AudioUnit buffer-list size overflow".to_string())?;
        let words = bytes.div_ceil(size_of::<usize>());
        let mut storage = Self {
            words: vec![0; words].into_boxed_slice(),
            channels,
        };
        // SAFETY: `words` provides pointer alignment and enough initialized
        // storage for the variable-length AudioBufferList plus all buffers.
        unsafe {
            let list = storage.as_mut_ptr().as_mut();
            list.mNumberBuffers = channels as u32;
            let buffers = slice::from_raw_parts_mut(list.mBuffers.as_mut_ptr(), channels);
            for (channel, buffer) in buffers.iter_mut().enumerate() {
                *buffer = AudioBuffer {
                    mNumberChannels: 1,
                    mDataByteSize: (max_block_frames * size_of::<f32>()) as u32,
                    mData: planar_storage
                        .as_mut_ptr()
                        .add(channel * max_block_frames)
                        .cast(),
                };
            }
        }
        Ok(storage)
    }

    fn as_mut_ptr(&mut self) -> NonNull<AudioBufferList> {
        // SAFETY: Boxed storage is non-null and aligned to at least pointer
        // alignment, which satisfies AudioBufferList on supported macOS ABIs.
        unsafe { NonNull::new_unchecked(self.words.as_mut_ptr().cast()) }
    }

    fn set_frame_count(&mut self, frames: usize) {
        let bytes = frames.saturating_mul(size_of::<f32>()) as u32;
        // SAFETY: Construction initialized exactly `channels` trailing buffers.
        unsafe {
            let list = self.as_mut_ptr().as_mut();
            let buffers = slice::from_raw_parts_mut(list.mBuffers.as_mut_ptr(), self.channels);
            for buffer in buffers {
                buffer.mDataByteSize = bytes;
            }
        }
    }
}

pub(super) struct AudioUnitBackend {
    instance: AudioUnit,
    _input_state: Box<AudioUnitInputState>,
    output_buffers: AudioBufferListStorage,
    metadata: NativePluginMetadata,
    parameters: Vec<Parameter>,
    parameter_bindings: Vec<AudioUnitParameterBinding>,
    sample_rate: u32,
    sample_position: u64,
    input_storage: Vec<f32>,
    output_storage: Vec<f32>,
    max_block_frames: usize,
    initialized: bool,
}

// SAFETY: The instance is exclusively accessed through `&mut`; the render
// callback is synchronous on that same processing thread and only reads the
// fixed input-state pointer retained by this backend.
unsafe impl Send for AudioUnitBackend {}

impl AudioUnitBackend {
    pub(super) fn load(
        descriptor: &PluginDescriptor,
        sample_rate: u32,
        max_block_frames: usize,
    ) -> Result<Self, String> {
        if descriptor.is_instrument || descriptor.audio_inputs == 0 {
            return Err(format!(
                "AudioUnit '{}' is an instrument; SOTF external hosting currently supports effect Audio Units",
                descriptor.name
            ));
        }
        let (component, description, component_name, version) = find_component(descriptor)?;
        let mut instance = ptr::null_mut();
        // SAFETY: The selected AudioComponent remains registered for the process
        // lifetime and CoreAudio initializes the out pointer on success.
        unsafe {
            ensure_status(
                AudioComponentInstanceNew(component, NonNull::from(&mut instance)),
                &descriptor.name,
                "instantiate AudioUnit",
            )?;
        }
        if instance.is_null() {
            return Err(format!(
                "AudioUnit '{}' returned a null component instance",
                descriptor.name
            ));
        }

        let input_channels = descriptor.audio_inputs;
        let output_channels = descriptor.audio_outputs.max(1);
        let input_storage = vec![0.0; input_channels * max_block_frames];
        let mut output_storage = vec![0.0; output_channels * max_block_frames];
        let mut input_state = Box::new(AudioUnitInputState {
            storage: input_storage.as_ptr(),
            channels: input_channels,
            max_frames: max_block_frames,
        });
        let output_buffers =
            AudioBufferListStorage::new(output_channels, max_block_frames, &mut output_storage)?;

        let setup = (|| {
            set_property(
                instance,
                kAudioUnitProperty_MaximumFramesPerSlice,
                kAudioUnitScope_Global,
                0,
                &(max_block_frames as u32),
                &descriptor.name,
                "set maximum frames per slice",
            )?;
            let input_format = planar_f32_format(sample_rate, input_channels)?;
            let output_format = planar_f32_format(sample_rate, output_channels)?;
            set_property(
                instance,
                kAudioUnitProperty_StreamFormat,
                kAudioUnitScope_Input,
                0,
                &input_format,
                &descriptor.name,
                "set input stream format",
            )?;
            set_property(
                instance,
                kAudioUnitProperty_StreamFormat,
                kAudioUnitScope_Output,
                0,
                &output_format,
                &descriptor.name,
                "set output stream format",
            )?;
            let callback = AURenderCallbackStruct {
                inputProc: Some(input_render_callback),
                inputProcRefCon: (&mut *input_state as *mut AudioUnitInputState).cast(),
            };
            set_property(
                instance,
                kAudioUnitProperty_SetRenderCallback,
                kAudioUnitScope_Input,
                0,
                &callback,
                &descriptor.name,
                "install input render callback",
            )?;
            // SAFETY: All stream formats, callback storage, and maximum frame
            // count have been configured and remain valid for the instance.
            unsafe {
                ensure_status(
                    AudioUnitInitialize(instance),
                    &descriptor.name,
                    "initialize AudioUnit",
                )
            }
        })();
        if let Err(error) = setup {
            // SAFETY: Instance creation succeeded and disposal accepts an
            // uninitialized or partially configured instance.
            unsafe {
                let _ = AudioComponentInstanceDispose(instance);
            }
            return Err(error);
        }

        let (vendor, name) = split_component_name(&component_name);
        let metadata = NativePluginMetadata {
            id: component_id(description),
            name,
            vendor: if vendor.is_empty() {
                descriptor.vendor.clone()
            } else {
                vendor
            },
            version,
            input_channels,
            output_channels,
        };
        let (parameters, parameter_bindings) = match collect_parameters(instance, &metadata.name) {
            Ok(parameters) => parameters,
            Err(error) => {
                // SAFETY: Parameter discovery happens after initialization, so
                // unwind the complete instance lifecycle on metadata failure.
                unsafe {
                    let _ = AudioUnitUninitialize(instance);
                    let _ = AudioComponentInstanceDispose(instance);
                }
                return Err(error);
            }
        };
        // Keep the variable alive until after `input_state` captured its stable
        // heap allocation. The Vec allocation itself never moves after here.
        input_state.storage = input_storage.as_ptr();
        Ok(Self {
            instance,
            _input_state: input_state,
            output_buffers,
            metadata,
            parameters,
            parameter_bindings,
            sample_rate,
            sample_position: 0,
            input_storage,
            output_storage,
            max_block_frames,
            initialized: true,
        })
    }
}

impl NativeExternalPluginBackend for AudioUnitBackend {
    fn metadata(&self) -> &NativePluginMetadata {
        &self.metadata
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.parameters.clone()
    }

    fn set_parameter(&mut self, id: &ParameterId, value: &ParameterValue) -> Result<(), String> {
        let binding = self
            .parameter_bindings
            .iter()
            .find(|binding| &binding.host_id == id)
            .ok_or_else(|| format!("AudioUnit parameter '{id}' is not exposed"))?;
        let plain = audio_unit_parameter_to_plain(value, binding.kind).ok_or_else(|| {
            format!("AudioUnit parameter '{id}' received incompatible value {value}")
        })?;
        // SAFETY: The instance is live and the parameter metadata declared this
        // global parameter writable. Offset zero applies it immediately.
        unsafe {
            ensure_status(
                AudioUnitSetParameter(
                    self.instance,
                    binding.audio_unit_id,
                    kAudioUnitScope_Global,
                    0,
                    plain,
                    0,
                ),
                &self.metadata.name,
                "set parameter",
            )
        }
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let binding = self
            .parameter_bindings
            .iter()
            .find(|binding| &binding.host_id == id)?;
        let mut value = 0.0_f32;
        // SAFETY: The instance is live and `value` is valid result storage.
        let status = unsafe {
            AudioUnitGetParameter(
                self.instance,
                binding.audio_unit_id,
                kAudioUnitScope_Global,
                0,
                NonNull::from(&mut value),
            )
        };
        if status != NO_ERR {
            return None;
        }
        plain_to_audio_unit_parameter(value, binding.kind)
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        input_channels: usize,
        output_channels: usize,
        context: &crate::plugin::ProcessContext,
    ) -> Result<(), String> {
        let frames = context.num_frames;
        if frames > self.max_block_frames {
            return Err(format!(
                "AudioUnit '{}' received {frames} frames, exceeding its configured maximum {}",
                self.metadata.name, self.max_block_frames,
            ));
        }
        if input_channels != self.metadata.input_channels
            || output_channels != self.metadata.output_channels
        {
            return Err(format!(
                "AudioUnit '{}' channel contract changed from {}→{} to {input_channels}→{output_channels} without rebuild",
                self.metadata.name, self.metadata.input_channels, self.metadata.output_channels
            ));
        }
        validate_au_event_contract(context, &self.parameter_bindings, &self.metadata.name)?;
        for frame in 0..frames {
            for channel in 0..input_channels {
                self.input_storage[channel * self.max_block_frames + frame] =
                    input[frame * input_channels + channel];
            }
        }
        for channel in 0..output_channels {
            self.output_storage
                [channel * self.max_block_frames..channel * self.max_block_frames + frames]
                .fill(0.0);
        }
        for event in context.midi_events {
            let data = event.message.data;
            let status = unsafe {
                MusicDeviceMIDIEvent(
                    self.instance,
                    u32::from(data[0]),
                    u32::from(data[1]),
                    u32::from(data[2]),
                    event.sample_offset as u32,
                )
            };
            ensure_status(status, &self.metadata.name, "schedule MIDI event")?;
        }
        for event in context.parameter_events {
            let binding = self
                .parameter_bindings
                .iter()
                .find(|binding| binding.host_id == event.parameter_id)
                .expect("validated AudioUnit parameter binding");
            let plain = audio_unit_parameter_to_plain(&event.value, binding.kind)
                .expect("validated AudioUnit automation value");
            let status = unsafe {
                AudioUnitSetParameter(
                    self.instance,
                    binding.audio_unit_id,
                    kAudioUnitScope_Global,
                    0,
                    plain,
                    event.sample_offset as u32,
                )
            };
            ensure_status(status, &self.metadata.name, "schedule parameter automation")?;
        }
        self.output_buffers.set_frame_count(frames);
        let mut flags = AudioUnitRenderActionFlags(0);
        // SAFETY: AudioTimeStamp is a plain C structure; zero is valid for all
        // unused fields and the sample-time flag marks the populated field.
        let mut timestamp = unsafe { std::mem::zeroed::<AudioTimeStamp>() };
        timestamp.mSampleTime = au_transport_sample_time(context);
        timestamp.mFlags = AudioTimeStampFlags::SampleTimeValid;
        // SAFETY: The initialized instance synchronously invokes the retained
        // input callback and renders into the fixed output AudioBufferList.
        let status = unsafe {
            AudioUnitRender(
                self.instance,
                &mut flags,
                NonNull::from(&mut timestamp),
                0,
                frames as u32,
                self.output_buffers.as_mut_ptr(),
            )
        };
        ensure_status(status, &self.metadata.name, "render audio")?;
        for frame in 0..frames {
            for channel in 0..output_channels {
                output[frame * output_channels + channel] =
                    self.output_storage[channel * self.max_block_frames + frame];
            }
        }
        self.sample_position = context
            .transport
            .sample_position
            .saturating_add(frames as u64);
        Ok(())
    }

    fn save_state(&self) -> Result<Option<Vec<u8>>, String> {
        let mut property_list_ptr: *const CFPropertyList = ptr::null();
        let mut size = size_of::<*const CFPropertyList>() as u32;
        // SAFETY: ClassInfo returns a retained CFPropertyList reference into the
        // exact pointer-sized result storage supplied here.
        unsafe {
            ensure_status(
                AudioUnitGetProperty(
                    self.instance,
                    kAudioUnitProperty_ClassInfo,
                    kAudioUnitScope_Global,
                    0,
                    NonNull::from(&mut property_list_ptr).cast(),
                    NonNull::from(&mut size),
                ),
                &self.metadata.name,
                "save class info",
            )?;
        }
        if size as usize != size_of::<*const CFPropertyList>() {
            return Err(format!(
                "AudioUnit '{}' returned malformed ClassInfo size {size}",
                self.metadata.name
            ));
        }
        let property_list_ptr = NonNull::new(property_list_ptr.cast_mut())
            .ok_or_else(|| format!("AudioUnit '{}' returned null ClassInfo", self.metadata.name))?;
        // SAFETY: kAudioUnitProperty_ClassInfo follows the Copy rule and returns
        // a +1 retained property-list reference owned by the host.
        let property_list = unsafe { CFRetained::<CFPropertyList>::from_raw(property_list_ptr) };
        let mut error_ptr: *mut CFError = ptr::null_mut();
        // SAFETY: The retained object is a valid property list and CoreFoundation
        // synchronously returns owned serialized data or an owned error.
        let data = unsafe {
            CFPropertyListCreateData(
                None,
                Some(&property_list),
                CFPropertyListFormat::BinaryFormat_v1_0,
                0,
                &mut error_ptr,
            )
        };
        release_cf_error(error_ptr);
        let data = data.ok_or_else(|| {
            format!(
                "AudioUnit '{}' ClassInfo could not be serialized",
                self.metadata.name
            )
        })?;
        Ok(Some(data.to_vec()))
    }

    fn load_state(&mut self, state: &[u8]) -> Result<(), String> {
        let data = CFData::from_bytes(state);
        let mut format = CFPropertyListFormat::BinaryFormat_v1_0;
        let mut error_ptr: *mut CFError = ptr::null_mut();
        // SAFETY: `data` remains live for the synchronous parser; result and
        // optional error references are returned at +1 retain counts.
        let property_list = unsafe {
            CFPropertyListCreateWithData(None, Some(&data), 0, &mut format, &mut error_ptr)
        };
        release_cf_error(error_ptr);
        let property_list = property_list.ok_or_else(|| {
            format!(
                "AudioUnit '{}' state is not a valid property list",
                self.metadata.name
            )
        })?;

        // SAFETY: State changes occur outside rendering. AudioUnit requires
        // uninitialization before mutating ClassInfo on arbitrary components.
        unsafe {
            ensure_status(
                AudioUnitUninitialize(self.instance),
                &self.metadata.name,
                "uninitialize for state restore",
            )?;
        }
        self.initialized = false;
        let property_list_ptr: *const CFPropertyList = &*property_list;
        // SAFETY: ClassInfo expects a pointer to one live CFPropertyListRef and
        // consumes neither the pointer nor its retain count.
        let load_result = unsafe {
            ensure_status(
                AudioUnitSetProperty(
                    self.instance,
                    kAudioUnitProperty_ClassInfo,
                    kAudioUnitScope_Global,
                    0,
                    (&property_list_ptr as *const *const CFPropertyList).cast(),
                    size_of::<*const CFPropertyList>() as u32,
                ),
                &self.metadata.name,
                "restore class info",
            )
        };
        // SAFETY: Stream formats and callbacks remain configured across
        // uninitialization, so the instance can be initialized again.
        let initialize_result = unsafe {
            ensure_status(
                AudioUnitInitialize(self.instance),
                &self.metadata.name,
                "reinitialize after state restore",
            )
        };
        if initialize_result.is_ok() {
            self.initialized = true;
        }
        match (load_result, initialize_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(load), Ok(())) => Err(load),
            (Ok(()), Err(initialize)) => Err(initialize),
            (Err(load), Err(initialize)) => Err(format!(
                "{load}; additionally failed to reinitialize: {initialize}"
            )),
        }
    }

    fn latency_samples(&self) -> usize {
        let mut latency = 0.0_f64;
        if get_property(
            self.instance,
            kAudioUnitProperty_Latency,
            kAudioUnitScope_Global,
            0,
            &mut latency,
        )
        .is_err()
            || !latency.is_finite()
            || latency <= 0.0
        {
            0
        } else {
            (latency * f64::from(self.sample_rate)).round() as usize
        }
    }
}

impl Drop for AudioUnitBackend {
    fn drop(&mut self) {
        // SAFETY: Reverse AudioUnit lifecycle for the exclusively owned live
        // instance. Callback state outlives uninitialization and disposal.
        unsafe {
            if self.initialized {
                let _ = AudioUnitUninitialize(self.instance);
                self.initialized = false;
            }
            let _ = AudioComponentInstanceDispose(self.instance);
        }
    }
}

fn collect_parameters(
    instance: AudioUnit,
    plugin_name: &str,
) -> Result<(Vec<Parameter>, Vec<AudioUnitParameterBinding>), String> {
    let mut byte_size = 0_u32;
    // SAFETY: The live instance synchronously writes the property byte size.
    let status = unsafe {
        AudioUnitGetPropertyInfo(
            instance,
            kAudioUnitProperty_ParameterList,
            kAudioUnitScope_Global,
            0,
            &mut byte_size,
            ptr::null_mut(),
        )
    };
    if status != NO_ERR {
        return Ok((Vec::new(), Vec::new()));
    }
    if !(byte_size as usize).is_multiple_of(size_of::<u32>()) {
        return Err(format!(
            "AudioUnit '{plugin_name}' returned malformed parameter-list size {byte_size}"
        ));
    }
    let mut ids = vec![0_u32; byte_size as usize / size_of::<u32>()];
    if !ids.is_empty() {
        let mut returned_size = byte_size;
        // SAFETY: `ids` provides exactly the storage size reported above.
        unsafe {
            ensure_status(
                AudioUnitGetProperty(
                    instance,
                    kAudioUnitProperty_ParameterList,
                    kAudioUnitScope_Global,
                    0,
                    NonNull::new_unchecked(ids.as_mut_ptr().cast()),
                    NonNull::from(&mut returned_size),
                ),
                plugin_name,
                "read parameter list",
            )?;
        }
        if returned_size != byte_size {
            return Err(format!(
                "AudioUnit '{plugin_name}' changed parameter-list size from {byte_size} to {returned_size}"
            ));
        }
    }

    let mut parameters = Vec::with_capacity(ids.len());
    let mut bindings = Vec::with_capacity(ids.len());
    for audio_unit_id in ids {
        // SAFETY: The type is a C property structure and zero initializes its
        // optional CF pointers and scalar fields before CoreAudio fills it.
        let mut info = unsafe { std::mem::zeroed::<AudioUnitParameterInfo>() };
        let mut info_size = size_of::<AudioUnitParameterInfo>() as u32;
        // SAFETY: The live instance writes one parameter-info structure for the
        // requested ID into exact-size caller-owned storage.
        let status = unsafe {
            AudioUnitGetProperty(
                instance,
                kAudioUnitProperty_ParameterInfo,
                kAudioUnitScope_Global,
                audio_unit_id,
                NonNull::from(&mut info).cast(),
                NonNull::from(&mut info_size),
            )
        };
        if status != NO_ERR || info_size as usize != size_of::<AudioUnitParameterInfo>() {
            continue;
        }
        let writable = info
            .flags
            .contains(AudioUnitParameterOptions::Flag_IsWritable);
        let readable = info
            .flags
            .contains(AudioUnitParameterOptions::Flag_IsReadable);
        if !writable || !readable {
            release_parameter_name_if_owned(&info);
            continue;
        }
        let host_id = format!("au.{audio_unit_id}");
        let name = bounded_c_char_array(&info.name);
        let name = if name.is_empty() {
            host_id.clone()
        } else {
            name
        };
        let kind = if info.unit == AudioUnitParameterUnit::Boolean {
            AudioUnitParameterKind::Boolean
        } else if info.unit == AudioUnitParameterUnit::Indexed
            && info.minValue.fract() == 0.0
            && info.maxValue.fract() == 0.0
            && info.defaultValue.fract() == 0.0
            && info.minValue >= i32::MIN as f32
            && info.maxValue <= i32::MAX as f32
        {
            AudioUnitParameterKind::Integer
        } else {
            AudioUnitParameterKind::Float
        };
        if !info.minValue.is_finite()
            || !info.maxValue.is_finite()
            || !info.defaultValue.is_finite()
            || info.minValue > info.maxValue
        {
            release_parameter_name_if_owned(&info);
            continue;
        }
        let mut parameter = match kind {
            AudioUnitParameterKind::Float => Parameter::new_float(
                &host_id,
                &name,
                info.defaultValue,
                info.minValue,
                info.maxValue,
            ),
            AudioUnitParameterKind::Integer => Parameter::new_int(
                &host_id,
                &name,
                info.defaultValue.round() as i32,
                info.minValue.round() as i32,
                info.maxValue.round() as i32,
            ),
            AudioUnitParameterKind::Boolean => {
                Parameter::new_bool(&host_id, &name, info.defaultValue >= 0.5)
            }
        };
        parameter.unit = audio_unit_parameter_unit(info.unit).to_string();
        let parameter_id = parameter.id.clone();
        parameters.push(parameter);
        bindings.push(AudioUnitParameterBinding {
            host_id: parameter_id,
            audio_unit_id,
            kind,
        });
        release_parameter_name_if_owned(&info);
    }
    Ok((parameters, bindings))
}

fn release_parameter_name_if_owned(info: &AudioUnitParameterInfo) {
    if info
        .flags
        .contains(AudioUnitParameterOptions::Flag_CFNameRelease)
        && !info.cfNameString.is_null()
    {
        // SAFETY: The AudioUnit parameter contract transfers a +1 CFName when
        // this flag is set; CFRetained releases it at the end of this scope.
        unsafe {
            drop(CFRetained::<CFString>::from_raw(NonNull::new_unchecked(
                info.cfNameString.cast_mut(),
            )));
        }
    }
}

fn bounded_c_char_array<const N: usize>(chars: &[std::ffi::c_char; N]) -> String {
    let nul = chars.iter().position(|value| *value == 0).unwrap_or(N);
    let bytes = chars[..nul]
        .iter()
        .map(|value| *value as u8)
        .collect::<Vec<_>>();
    String::from_utf8_lossy(&bytes).trim().to_string()
}

fn audio_unit_parameter_unit(unit: AudioUnitParameterUnit) -> &'static str {
    if unit == AudioUnitParameterUnit::Percent {
        "%"
    } else if unit == AudioUnitParameterUnit::Seconds {
        "s"
    } else if unit == AudioUnitParameterUnit::Milliseconds {
        "ms"
    } else if unit == AudioUnitParameterUnit::Hertz {
        "Hz"
    } else if unit == AudioUnitParameterUnit::Decibels {
        "dB"
    } else if unit == AudioUnitParameterUnit::Degrees {
        "°"
    } else if unit == AudioUnitParameterUnit::BPM {
        "BPM"
    } else if unit == AudioUnitParameterUnit::Meters {
        "m"
    } else {
        ""
    }
}

fn audio_unit_parameter_to_plain(
    value: &ParameterValue,
    kind: AudioUnitParameterKind,
) -> Option<f32> {
    match (kind, value) {
        (AudioUnitParameterKind::Float, ParameterValue::Float(value)) => Some(*value),
        (AudioUnitParameterKind::Integer, ParameterValue::Int(value)) => Some(*value as f32),
        (AudioUnitParameterKind::Boolean, ParameterValue::Bool(value)) => {
            Some(f32::from(u8::from(*value)))
        }
        _ => None,
    }
}

fn validate_au_event_contract(
    context: &crate::plugin::ProcessContext,
    bindings: &[AudioUnitParameterBinding],
    plugin_name: &str,
) -> Result<(), String> {
    for event in context.midi_events {
        if event.sample_offset >= context.num_frames {
            return Err(format!(
                "AudioUnit '{plugin_name}' received MIDI offset {} outside a {}-frame block",
                event.sample_offset, context.num_frames
            ));
        }
    }
    for event in context.parameter_events {
        if event.sample_offset >= context.num_frames {
            return Err(format!(
                "AudioUnit '{plugin_name}' received automation offset {} outside a {}-frame block",
                event.sample_offset, context.num_frames
            ));
        }
        let binding = bindings
            .iter()
            .find(|binding| binding.host_id == event.parameter_id)
            .ok_or_else(|| {
                format!(
                    "AudioUnit '{plugin_name}' has no automated parameter '{}'",
                    event.parameter_id
                )
            })?;
        if audio_unit_parameter_to_plain(&event.value, binding.kind).is_none() {
            return Err(format!(
                "AudioUnit '{plugin_name}' rejected automation value for '{}'",
                event.parameter_id
            ));
        }
    }
    Ok(())
}

fn au_transport_sample_time(context: &crate::plugin::ProcessContext) -> f64 {
    context.transport.sample_position as f64
}

fn plain_to_audio_unit_parameter(
    value: f32,
    kind: AudioUnitParameterKind,
) -> Option<ParameterValue> {
    if !value.is_finite() {
        return None;
    }
    match kind {
        AudioUnitParameterKind::Float => Some(ParameterValue::Float(value)),
        AudioUnitParameterKind::Integer => Some(ParameterValue::Int(
            value.round().clamp(i32::MIN as f32, i32::MAX as f32) as i32,
        )),
        AudioUnitParameterKind::Boolean => Some(ParameterValue::Bool(value >= 0.5)),
    }
}

fn find_component(
    requested: &PluginDescriptor,
) -> Result<(AudioComponent, AudioComponentDescription, String, String), String> {
    if let Some(description) = parse_component_id(&requested.id) {
        // SAFETY: Exact non-wildcard component lookup borrows the local C
        // description only for the synchronous registry call.
        let component =
            unsafe { AudioComponentFindNext(ptr::null_mut(), NonNull::from(&description)) };
        if component.is_null() {
            return Err(format!(
                "AudioUnit '{}' with component ID '{}' is not registered",
                requested.name, requested.id
            ));
        }
        return component_details(component, description);
    }

    let requested_stem = requested
        .path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let mut matches = Vec::new();
    let mut available = Vec::new();
    for component_type in [
        kAudioUnitType_Effect,
        kAudioUnitType_MusicEffect,
        kAudioUnitType_FormatConverter,
    ] {
        let search = AudioComponentDescription {
            componentType: component_type,
            componentSubType: 0,
            componentManufacturer: 0,
            componentFlags: 0,
            componentFlagsMask: 0,
        };
        let mut previous = ptr::null_mut();
        for _ in 0..MAX_COMPONENTS {
            // SAFETY: Registry iteration accepts the previous opaque component
            // and synchronously borrows the local search description.
            let component = unsafe { AudioComponentFindNext(previous, NonNull::from(&search)) };
            if component.is_null() {
                break;
            }
            previous = component;
            let details = component_details(component, search)?;
            let full_name = &details.2;
            let (_, display_name) = split_component_name(full_name);
            let requested_id_name = requested.id.strip_prefix("au.").unwrap_or(&requested.id);
            let matches_requested = full_name.eq_ignore_ascii_case(&requested.name)
                || display_name.eq_ignore_ascii_case(&requested.name)
                || display_name.eq_ignore_ascii_case(requested_stem)
                || display_name.eq_ignore_ascii_case(requested_id_name);
            available.push(format!("{} ({})", display_name, component_id(details.1)));
            if matches_requested {
                matches.push(details);
            }
        }
    }
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(format!(
            "AudioUnit registry does not contain requested plugin '{}'/'{}' from '{}'; available effects: {}",
            requested.id,
            requested.name,
            requested.path.display(),
            if available.is_empty() {
                "<none>".to_string()
            } else {
                available.join(", ")
            }
        )),
        count => Err(format!(
            "AudioUnit registry contains {count} components matching '{}'/'{}'; use a resolved au.<type>.<subtype>.<manufacturer> ID",
            requested.id, requested.name
        )),
    }
}

fn component_details(
    component: AudioComponent,
    mut description: AudioComponentDescription,
) -> Result<(AudioComponent, AudioComponentDescription, String, String), String> {
    // SAFETY: The registry component is live, and all result storage remains
    // valid for the duration of the synchronous queries.
    unsafe {
        ensure_status(
            AudioComponentGetDescription(component, NonNull::from(&mut description)),
            "AudioUnit registry",
            "read component description",
        )?;
        let mut name_ptr: *const CFString = ptr::null();
        ensure_status(
            AudioComponentCopyName(component, NonNull::from(&mut name_ptr)),
            "AudioUnit registry",
            "read component name",
        )?;
        let name = NonNull::new(name_ptr.cast_mut())
            .map(|ptr| CFRetained::<CFString>::from_raw(ptr).to_string())
            .ok_or_else(|| "AudioUnit registry returned a null component name".to_string())?;
        let mut version = 0_u32;
        ensure_status(
            AudioComponentGetVersion(component, NonNull::from(&mut version)),
            &name,
            "read component version",
        )?;
        Ok((
            component,
            description,
            name,
            format_component_version(version),
        ))
    }
}

fn parse_component_id(id: &str) -> Option<AudioComponentDescription> {
    let mut parts = id.split('.');
    if parts.next()? != "au" {
        return None;
    }
    let component_type = u32::from_str_radix(parts.next()?, 16).ok()?;
    let subtype = u32::from_str_radix(parts.next()?, 16).ok()?;
    let manufacturer = u32::from_str_radix(parts.next()?, 16).ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(AudioComponentDescription {
        componentType: component_type,
        componentSubType: subtype,
        componentManufacturer: manufacturer,
        componentFlags: 0,
        componentFlagsMask: 0,
    })
}

fn component_id(description: AudioComponentDescription) -> String {
    format!(
        "au.{:08X}.{:08X}.{:08X}",
        description.componentType, description.componentSubType, description.componentManufacturer
    )
}

fn split_component_name(full_name: &str) -> (String, String) {
    full_name.split_once(':').map_or_else(
        || (String::new(), full_name.trim().to_string()),
        |(vendor, name)| (vendor.trim().to_string(), name.trim().to_string()),
    )
}

fn format_component_version(version: u32) -> String {
    format!(
        "{}.{}.{}",
        version >> 16,
        (version >> 8) & 0xff,
        version & 0xff
    )
}

fn planar_f32_format(
    sample_rate: u32,
    channels: usize,
) -> Result<AudioStreamBasicDescription, String> {
    let channels = u32::try_from(channels)
        .map_err(|_| "AudioUnit channel count does not fit the CoreAudio ABI".to_string())?;
    Ok(AudioStreamBasicDescription {
        mSampleRate: f64::from(sample_rate),
        mFormatID: kAudioFormatLinearPCM,
        mFormatFlags: kAudioFormatFlagIsFloat
            | kAudioFormatFlagIsPacked
            | kAudioFormatFlagIsNonInterleaved,
        mBytesPerPacket: size_of::<f32>() as u32,
        mFramesPerPacket: 1,
        mBytesPerFrame: size_of::<f32>() as u32,
        mChannelsPerFrame: channels,
        mBitsPerChannel: 32,
        mReserved: 0,
    })
}

fn set_property<T>(
    instance: AudioUnit,
    property: u32,
    scope: u32,
    element: u32,
    value: &T,
    plugin: &str,
    operation: &str,
) -> Result<(), String> {
    // SAFETY: `value` is a live C-compatible property payload of the declared
    // size and the instance remains owned by the backend.
    unsafe {
        ensure_status(
            AudioUnitSetProperty(
                instance,
                property,
                scope,
                element,
                (value as *const T).cast(),
                size_of::<T>() as u32,
            ),
            plugin,
            operation,
        )
    }
}

fn get_property<T>(
    instance: AudioUnit,
    property: u32,
    scope: u32,
    element: u32,
    value: &mut T,
) -> Result<(), i32> {
    let mut size = size_of::<T>() as u32;
    // SAFETY: `value` provides writable storage of the exact declared size and
    // the instance remains live for the synchronous query.
    let status = unsafe {
        AudioUnitGetProperty(
            instance,
            property,
            scope,
            element,
            NonNull::from(value).cast(),
            NonNull::from(&mut size),
        )
    };
    if status == NO_ERR && size as usize == size_of::<T>() {
        Ok(())
    } else {
        Err(status)
    }
}

fn ensure_status(status: i32, plugin: &str, operation: &str) -> Result<(), String> {
    if status == NO_ERR {
        Ok(())
    } else {
        Err(format!(
            "AudioUnit '{plugin}' failed to {operation} (OSStatus {status})"
        ))
    }
}

fn release_cf_error(error: *mut CFError) {
    if let Some(error) = NonNull::new(error) {
        // SAFETY: CoreFoundation property-list APIs return errors at +1 retain
        // count; wrapping and dropping releases the optional result.
        unsafe {
            drop(CFRetained::<CFError>::from_raw(error));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{MidiEvent, MidiMessage, ParameterEvent, ProcessContext, TransportInfo};

    #[test]
    fn au_event_and_transport_fixture_preserves_offsets_types_and_sample_time() {
        let bindings = [AudioUnitParameterBinding {
            host_id: ParameterId::from("gain"),
            audio_unit_id: 17,
            kind: AudioUnitParameterKind::Float,
        }];
        let midi = [MidiEvent::new(13, MidiMessage::note_on(2, 64, 99))];
        let automation = [ParameterEvent::new(
            47,
            ParameterId::from("gain"),
            ParameterValue::Float(0.75),
        )];
        let context = ProcessContext::new(48_000, 64)
            .with_transport(TransportInfo::at_sample(123_456, 48_000).with_tempo(91.0, 48_000))
            .with_all_events(&midi, &[], &automation);
        validate_au_event_contract(&context, &bindings, "fixture").unwrap();
        assert_eq!(midi[0].sample_offset, 13);
        assert_eq!(automation[0].sample_offset, 47);
        assert_eq!(
            audio_unit_parameter_to_plain(&automation[0].value, bindings[0].kind),
            Some(0.75)
        );
        assert_eq!(au_transport_sample_time(&context), 123_456.0);
    }

    #[test]
    fn au_event_fixture_rejects_out_of_block_and_wrong_parameter_types() {
        let bindings = [AudioUnitParameterBinding {
            host_id: ParameterId::from("enabled"),
            audio_unit_id: 3,
            kind: AudioUnitParameterKind::Boolean,
        }];
        let invalid_offset = [MidiEvent::new(64, MidiMessage::note_on(0, 60, 1))];
        let context = ProcessContext::new(48_000, 64).with_midi_events(&invalid_offset);
        assert!(validate_au_event_contract(&context, &bindings, "fixture").is_err());

        let wrong_type = [ParameterEvent::new(
            1,
            ParameterId::from("enabled"),
            ParameterValue::Float(1.0),
        )];
        let context = ProcessContext::new(48_000, 64).with_parameter_events(&wrong_type);
        assert!(validate_au_event_contract(&context, &bindings, "fixture").is_err());
    }
}
