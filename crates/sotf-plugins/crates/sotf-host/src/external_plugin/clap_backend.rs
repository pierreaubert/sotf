use super::native_backend::{NativeExternalPluginBackend, NativePluginMetadata};
use super::plugin_descriptor::{PluginDescriptor, resolve_dynamic_library_path};
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use clap_sys::audio_buffer::clap_audio_buffer;
use clap_sys::entry::clap_plugin_entry;
use clap_sys::events::{
    CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_IS_LIVE, CLAP_EVENT_MIDI, CLAP_EVENT_PARAM_VALUE,
    CLAP_TRANSPORT_HAS_BEATS_TIMELINE, CLAP_TRANSPORT_HAS_SECONDS_TIMELINE,
    CLAP_TRANSPORT_HAS_TEMPO, CLAP_TRANSPORT_HAS_TIME_SIGNATURE, CLAP_TRANSPORT_IS_LOOP_ACTIVE,
    CLAP_TRANSPORT_IS_PLAYING, CLAP_TRANSPORT_IS_RECORDING, clap_event_midi,
    clap_event_param_value, clap_event_transport,
};
use clap_sys::events::{clap_event_header, clap_input_events, clap_output_events};
use clap_sys::ext::audio_ports::{
    CLAP_EXT_AUDIO_PORTS, clap_audio_port_info, clap_plugin_audio_ports,
};
use clap_sys::ext::latency::{CLAP_EXT_LATENCY, clap_plugin_latency};
use clap_sys::ext::params::{
    CLAP_EXT_PARAMS, CLAP_PARAM_IS_HIDDEN, CLAP_PARAM_IS_READONLY, CLAP_PARAM_IS_STEPPED,
    clap_param_info, clap_plugin_params,
};
use clap_sys::ext::state::{CLAP_EXT_STATE, clap_plugin_state};
use clap_sys::factory::plugin_factory::{CLAP_PLUGIN_FACTORY_ID, clap_plugin_factory};
use clap_sys::fixedpoint::{CLAP_BEATTIME_FACTOR, CLAP_SECTIME_FACTOR};
use clap_sys::host::clap_host;
use clap_sys::plugin::{clap_plugin, clap_plugin_descriptor};
use clap_sys::process::{CLAP_PROCESS_ERROR, clap_process};
use clap_sys::stream::{clap_istream, clap_ostream};
use clap_sys::version::{CLAP_VERSION, clap_version_is_compatible};
use libloading::Library;
use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_void};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

const MAX_EXPOSED_PARAMETERS: u32 = 16_384;

#[derive(Clone, Copy)]
enum ClapParameterKind {
    Float,
    Int,
    Bool,
}

struct ClapParameterBinding {
    host_id: ParameterId,
    clap_id: u32,
    cookie: *mut c_void,
    kind: ClapParameterKind,
}

#[derive(Default)]
struct ClapHostRequests {
    restart: AtomicBool,
    process: AtomicBool,
    callback: AtomicBool,
}

struct ClapInputEventLists<'a> {
    parameters: &'a [clap_event_param_value],
    automation: &'a [clap_event_param_value],
    midi: &'a [clap_event_midi],
}

struct ClapLibrary {
    _library: Library,
    entry: *const clap_plugin_entry,
}

// SAFETY: `entry` points into `_library`, which is never unloaded while a
// `ClapLibrary` is reachable. Entry/factory calls are only made during
// serialized instance construction; plugin processing uses the per-instance
// `clap_plugin` pointer instead.
unsafe impl Send for ClapLibrary {}
// SAFETY: The same lifetime invariant applies across threads. The CLAP entry
// is immutable after the library has initialized.
unsafe impl Sync for ClapLibrary {}

fn library_registry() -> &'static Mutex<HashMap<PathBuf, Arc<ClapLibrary>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<PathBuf, Arc<ClapLibrary>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

impl ClapLibrary {
    fn load(path: &Path) -> Result<Arc<Self>, String> {
        let path = path.canonicalize().map_err(|error| {
            format!(
                "failed to canonicalize CLAP library '{}': {error}",
                path.display()
            )
        })?;
        let mut registry = library_registry()
            .lock()
            .map_err(|_| "CLAP library registry mutex is poisoned".to_string())?;
        if let Some(library) = registry.get(&path) {
            return Ok(Arc::clone(library));
        }

        // SAFETY: Loading executes the plugin library's platform initializer.
        // The canonical path came from the validated descriptor, the resulting
        // handle is kept alive in the process-wide registry, and every symbol
        // is checked before it is dereferenced.
        let library = unsafe { Library::new(&path) }.map_err(|error| {
            format!(
                "failed to load CLAP plugin library '{}': {error}",
                path.display()
            )
        })?;
        // SAFETY: `clap_entry` is the required CLAP entry symbol. The pointer is
        // only dereferenced while `library` is alive and after a null check.
        let entry = unsafe {
            *library
                .get::<*const clap_plugin_entry>(b"clap_entry\0")
                .map_err(|error| {
                    format!(
                        "CLAP plugin '{}' is missing required symbol 'clap_entry': {error}",
                        path.display()
                    )
                })?
        };
        if entry.is_null() {
            return Err(format!(
                "CLAP plugin '{}' exported a null clap_entry",
                path.display()
            ));
        }

        // SAFETY: `entry` was resolved from the live library. `plugin_path`
        // remains valid for the entire call and CLAP requires `init` before any
        // factory access.
        unsafe {
            if !clap_version_is_compatible((*entry).clap_version) {
                return Err(format!(
                    "CLAP plugin '{}' uses incompatible CLAP version {}.{}.{}",
                    path.display(),
                    (*entry).clap_version.major,
                    (*entry).clap_version.minor,
                    (*entry).clap_version.revision
                ));
            }
            let init = (*entry).init.ok_or_else(|| {
                format!(
                    "CLAP plugin '{}' has no entry init callback",
                    path.display()
                )
            })?;
            let path_c =
                std::ffi::CString::new(path.to_string_lossy().as_bytes()).map_err(|_| {
                    format!(
                        "CLAP plugin path '{}' contains an interior NUL",
                        path.display()
                    )
                })?;
            if !init(path_c.as_ptr()) {
                return Err(format!(
                    "CLAP plugin '{}' rejected entry initialization",
                    path.display()
                ));
            }
        }

        // Keep initialized CLAP libraries loaded for the process lifetime. This
        // avoids racing entry deinitialization when several graph instances use
        // the same plugin library.
        let loaded = Arc::new(Self {
            _library: library,
            entry,
        });
        registry.insert(path, Arc::clone(&loaded));
        Ok(loaded)
    }
}

pub(super) struct ClapBackend {
    _library: Arc<ClapLibrary>,
    host: Box<clap_host>,
    host_requests: Box<ClapHostRequests>,
    plugin: *const clap_plugin,
    metadata: NativePluginMetadata,
    input_storage: Vec<f32>,
    output_storage: Vec<f32>,
    input_ptrs: Vec<*mut f32>,
    output_ptrs: Vec<*mut f32>,
    parameters: Vec<Parameter>,
    parameter_bindings: Vec<ClapParameterBinding>,
    pending_parameter_events: Vec<clap_event_param_value>,
    automation_events: Vec<clap_event_param_value>,
    midi_events: Vec<clap_event_midi>,
    max_block_frames: usize,
    steady_time: i64,
    active: bool,
    processing: bool,
}

// SAFETY: CLAP permits an activated instance to be processed on one audio
// thread. `ExternalPlugin` provides exclusive `&mut` access, and all raw
// pointers are instance-owned or point into the process-lifetime library.
unsafe impl Send for ClapBackend {}

struct ClapLifecycleGuard {
    plugin: *const clap_plugin,
    active: bool,
    processing: bool,
    armed: bool,
}

impl ClapLifecycleGuard {
    fn new(plugin: *const clap_plugin) -> Self {
        Self {
            plugin,
            active: false,
            processing: false,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ClapLifecycleGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // SAFETY: The guard is created immediately after `create_plugin` and
        // remains armed until `ClapBackend` assumes ownership. Its flags track
        // the successful CLAP lifecycle transitions and unwind them in reverse.
        unsafe {
            if self.processing
                && let Some(stop) = (*self.plugin).stop_processing
            {
                stop(self.plugin);
            }
            if self.active
                && let Some(deactivate) = (*self.plugin).deactivate
            {
                deactivate(self.plugin);
            }
            destroy_plugin(self.plugin);
        }
    }
}

impl ClapBackend {
    pub(super) fn load(
        descriptor: &PluginDescriptor,
        sample_rate: u32,
        max_block_frames: usize,
    ) -> Result<Self, String> {
        let library_path = resolve_dynamic_library_path(descriptor)?;
        let library = ClapLibrary::load(&library_path)?;
        let host_requests = Box::<ClapHostRequests>::default();
        let host = Box::new(clap_host {
            clap_version: CLAP_VERSION,
            host_data: (&*host_requests as *const ClapHostRequests)
                .cast_mut()
                .cast(),
            name: c"SOTF".as_ptr(),
            vendor: c"spinorama.org".as_ptr(),
            url: c"https://spinorama.org".as_ptr(),
            version: c"0.5".as_ptr(),
            get_extension: Some(host_get_extension),
            request_restart: Some(host_request_restart),
            request_process: Some(host_request_process),
            request_callback: Some(host_request_callback),
        });

        // SAFETY: The entry belongs to `library`, which remains retained by the
        // backend. Factory and descriptor pointers are plugin-owned immutable
        // data valid after entry initialization.
        let (plugin, metadata) = unsafe {
            let get_factory = (*library.entry).get_factory.ok_or_else(|| {
                format!(
                    "CLAP plugin '{}' has no get_factory callback",
                    library_path.display()
                )
            })?;
            let factory =
                get_factory(CLAP_PLUGIN_FACTORY_ID.as_ptr()).cast::<clap_plugin_factory>();
            if factory.is_null() {
                return Err(format!(
                    "CLAP plugin '{}' did not provide the plugin factory",
                    library_path.display()
                ));
            }
            let plugin_descriptor = select_plugin_descriptor(factory, descriptor, &library_path)?;
            let metadata = descriptor_metadata(plugin_descriptor)?;
            let create = (*factory).create_plugin.ok_or_else(|| {
                format!(
                    "CLAP plugin '{}' factory has no create callback",
                    library_path.display()
                )
            })?;
            let plugin = create(factory, host.as_ref(), (*plugin_descriptor).id);
            if plugin.is_null() {
                return Err(format!(
                    "CLAP factory '{}' could not create plugin '{}'",
                    library_path.display(),
                    metadata.id
                ));
            }
            (plugin, metadata)
        };

        let mut lifecycle = ClapLifecycleGuard::new(plugin);
        // SAFETY: `plugin` is a live factory-created instance. Reject inverse
        // lifecycle holes before entering any state that would require them.
        unsafe { validate_clap_lifecycle_callbacks(plugin, &metadata.name)? };
        // SAFETY: `plugin` was just created by the retained factory and has not
        // yet been exposed or used by another thread. `lifecycle` records each
        // completed transition so every error path unwinds correctly.
        let (input_channels, output_channels) = unsafe {
            Self::initialize_instance(
                plugin,
                &metadata,
                descriptor,
                sample_rate,
                max_block_frames,
                &mut lifecycle,
            )?
        };

        let mut metadata = metadata;
        metadata.input_channels = input_channels;
        metadata.output_channels = output_channels;
        // SAFETY: The plugin is initialized and retains its parameter extension
        // and cookies until destruction.
        let (parameters, parameter_bindings) = unsafe { query_parameters(plugin, &metadata)? };
        let pending_event_capacity = parameters.len().max(64);
        let mut backend = Self {
            _library: library,
            host,
            host_requests,
            plugin,
            metadata,
            input_storage: vec![0.0; input_channels.saturating_mul(max_block_frames)],
            output_storage: vec![0.0; output_channels.saturating_mul(max_block_frames)],
            input_ptrs: Vec::with_capacity(input_channels),
            output_ptrs: Vec::with_capacity(output_channels),
            parameters,
            parameter_bindings,
            pending_parameter_events: Vec::with_capacity(pending_event_capacity),
            automation_events: Vec::with_capacity(1024),
            midi_events: Vec::with_capacity(1024),
            max_block_frames,
            steady_time: 0,
            active: true,
            processing: true,
        };
        lifecycle.disarm();
        backend.rebuild_channel_pointers();
        Ok(backend)
    }

    unsafe fn initialize_instance(
        plugin: *const clap_plugin,
        metadata: &NativePluginMetadata,
        requested: &PluginDescriptor,
        sample_rate: u32,
        max_block_frames: usize,
        lifecycle: &mut ClapLifecycleGuard,
    ) -> Result<(usize, usize), String> {
        // SAFETY: The caller guarantees `plugin` came from the live CLAP
        // factory. Each callback is checked before use and called in the CLAP
        // lifecycle order.
        unsafe {
            let init = (*plugin)
                .init
                .ok_or_else(|| format!("CLAP plugin '{}' has no init callback", metadata.name))?;
            if !init(plugin) {
                return Err(format!(
                    "CLAP plugin '{}' rejected initialization",
                    metadata.name
                ));
            }
            let channels = query_audio_channels(plugin, requested, metadata)?;
            let activate = (*plugin).activate.ok_or_else(|| {
                format!("CLAP plugin '{}' has no activate callback", metadata.name)
            })?;
            if !activate(plugin, f64::from(sample_rate), 1, max_block_frames as u32) {
                return Err(format!(
                    "CLAP plugin '{}' rejected {} Hz activation with block range 1..={max_block_frames}",
                    metadata.name, sample_rate,
                ));
            }
            lifecycle.active = true;
            let start = (*plugin).start_processing.ok_or_else(|| {
                format!(
                    "CLAP plugin '{}' has no start_processing callback",
                    metadata.name
                )
            })?;
            if !start(plugin) {
                return Err(format!(
                    "CLAP plugin '{}' refused to start processing",
                    metadata.name
                ));
            }
            lifecycle.processing = true;
            Ok(channels)
        }
    }

    fn rebuild_channel_pointers(&mut self) {
        self.input_ptrs.clear();
        for channel in 0..self.metadata.input_channels {
            // SAFETY: Every channel owns a disjoint negotiated-size slice
            // in the fixed-capacity storage, which is never resized afterwards.
            self.input_ptrs.push(unsafe {
                self.input_storage
                    .as_mut_ptr()
                    .add(channel * self.max_block_frames)
            });
        }
        self.output_ptrs.clear();
        for channel in 0..self.metadata.output_channels {
            // SAFETY: Same invariant as input storage.
            self.output_ptrs.push(unsafe {
                self.output_storage
                    .as_mut_ptr()
                    .add(channel * self.max_block_frames)
            });
        }
    }
}

unsafe fn validate_clap_lifecycle_callbacks(
    plugin: *const clap_plugin,
    plugin_name: &str,
) -> Result<(), String> {
    // SAFETY: The caller guarantees a live factory-created CLAP instance.
    unsafe {
        for (callback, present) in [
            ("destroy", (*plugin).destroy.is_some()),
            ("activate", (*plugin).activate.is_some()),
            ("deactivate", (*plugin).deactivate.is_some()),
            ("start_processing", (*plugin).start_processing.is_some()),
            ("stop_processing", (*plugin).stop_processing.is_some()),
        ] {
            if !present {
                return Err(format!(
                    "CLAP plugin '{plugin_name}' has no {callback} callback"
                ));
            }
        }
    }
    Ok(())
}

impl NativeExternalPluginBackend for ClapBackend {
    fn metadata(&self) -> &NativePluginMetadata {
        &self.metadata
    }

    fn parameters(&self) -> Vec<Parameter> {
        self.parameters.clone()
    }

    fn set_parameter(&mut self, id: &ParameterId, value: &ParameterValue) -> Result<(), String> {
        let index = self
            .parameter_bindings
            .iter()
            .position(|binding| &binding.host_id == id)
            .ok_or_else(|| {
                format!(
                    "CLAP plugin '{}' has no parameter '{id}'",
                    self.metadata.name
                )
            })?;
        self.parameters[index].validate(value).map_err(|error| {
            format!(
                "CLAP plugin '{}' rejected parameter '{id}': {error}",
                self.metadata.name
            )
        })?;
        let binding = &self.parameter_bindings[index];
        let numeric = parameter_value_as_f64(value);
        self.pending_parameter_events.push(clap_event_param_value {
            header: clap_event_header {
                size: std::mem::size_of::<clap_event_param_value>() as u32,
                time: 0,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_PARAM_VALUE,
                flags: CLAP_EVENT_IS_LIVE,
            },
            param_id: binding.clap_id,
            cookie: binding.cookie,
            note_id: -1,
            port_index: -1,
            channel: -1,
            key: -1,
            value: numeric,
        });
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let binding = self
            .parameter_bindings
            .iter()
            .find(|binding| &binding.host_id == id)?;
        if let Some(pending) = self
            .pending_parameter_events
            .iter()
            .rev()
            .find(|event| event.param_id == binding.clap_id)
        {
            return Some(parameter_value_from_f64(binding.kind, pending.value));
        }

        // SAFETY: Parameter value lookup is a synchronous query against the
        // live initialized instance.
        unsafe {
            let params = plugin_extension::<clap_plugin_params>(self.plugin, CLAP_EXT_PARAMS)?;
            let get = (*params).get_value?;
            let mut value = 0.0;
            get(self.plugin, binding.clap_id, &mut value)
                .then(|| parameter_value_from_f64(binding.kind, value))
        }
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
                "CLAP plugin '{}' received {frames} frames, exceeding its activated maximum {}",
                self.metadata.name, self.max_block_frames,
            ));
        }
        if input_channels != self.metadata.input_channels
            || output_channels != self.metadata.output_channels
        {
            return Err(format!(
                "CLAP plugin '{}' channel contract changed from {}→{} to {input_channels}→{output_channels} without rebuild",
                self.metadata.name, self.metadata.input_channels, self.metadata.output_channels
            ));
        }

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

        self.midi_events.clear();
        self.automation_events.clear();
        if context.parameter_events.len() > self.automation_events.capacity() {
            return Err(format!(
                "CLAP plugin '{}' received {} automation events, exceeding the realtime capacity {}",
                self.metadata.name,
                context.parameter_events.len(),
                self.automation_events.capacity()
            ));
        }
        for event in context.parameter_events {
            if event.sample_offset >= frames {
                return Err(format!(
                    "CLAP plugin '{}' received automation offset {} outside a {frames}-frame block",
                    self.metadata.name, event.sample_offset
                ));
            }
            let binding = self
                .parameter_bindings
                .iter()
                .find(|binding| binding.host_id == event.parameter_id)
                .ok_or_else(|| {
                    format!(
                        "CLAP plugin '{}' has no automated parameter '{}'",
                        self.metadata.name, event.parameter_id
                    )
                })?;
            self.automation_events.push(clap_event_param_value {
                header: clap_event_header {
                    size: std::mem::size_of::<clap_event_param_value>() as u32,
                    time: event.sample_offset as u32,
                    space_id: CLAP_CORE_EVENT_SPACE_ID,
                    type_: CLAP_EVENT_PARAM_VALUE,
                    flags: CLAP_EVENT_IS_LIVE,
                },
                param_id: binding.clap_id,
                cookie: binding.cookie,
                note_id: -1,
                port_index: -1,
                channel: -1,
                key: -1,
                value: parameter_value_as_f64(&event.value),
            });
        }
        if context.midi_events.len() > self.midi_events.capacity() {
            return Err(format!(
                "CLAP plugin '{}' received {} MIDI events, exceeding the realtime capacity {}",
                self.metadata.name,
                context.midi_events.len(),
                self.midi_events.capacity()
            ));
        }
        for event in context.midi_events {
            if event.sample_offset >= frames || event.message.len == 0 {
                return Err(format!(
                    "CLAP plugin '{}' received MIDI offset {} outside a {frames}-frame block",
                    self.metadata.name, event.sample_offset
                ));
            }
            self.midi_events.push(clap_event_midi {
                header: clap_event_header {
                    size: std::mem::size_of::<clap_event_midi>() as u32,
                    time: event.sample_offset as u32,
                    space_id: CLAP_CORE_EVENT_SPACE_ID,
                    type_: CLAP_EVENT_MIDI,
                    flags: CLAP_EVENT_IS_LIVE,
                },
                port_index: 0,
                data: event.message.data,
            });
        }

        let input_buffer = clap_audio_buffer {
            data32: self.input_ptrs.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: input_channels as u32,
            latency: 0,
            constant_mask: 0,
        };
        let mut output_buffer = clap_audio_buffer {
            data32: self.output_ptrs.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: output_channels as u32,
            latency: 0,
            constant_mask: 0,
        };
        let event_lists = ClapInputEventLists {
            parameters: &self.pending_parameter_events,
            automation: &self.automation_events,
            midi: &self.midi_events,
        };
        let input_events = clap_input_events {
            ctx: (&event_lists as *const ClapInputEventLists)
                .cast_mut()
                .cast(),
            size: Some(input_event_count),
            get: Some(input_event_get),
        };
        let output_events = clap_output_events {
            ctx: ptr::null_mut(),
            try_push: Some(discard_output_event),
        };
        let transport = clap_transport(context);
        let process = clap_process {
            steady_time: self.steady_time,
            frames_count: frames as u32,
            transport: &transport,
            audio_inputs: if input_channels == 0 {
                ptr::null()
            } else {
                &input_buffer
            },
            audio_outputs: if output_channels == 0 {
                ptr::null_mut()
            } else {
                &mut output_buffer
            },
            audio_inputs_count: u32::from(input_channels != 0),
            audio_outputs_count: u32::from(output_channels != 0),
            in_events: &input_events,
            out_events: &output_events,
        };

        // SAFETY: The plugin is initialized, activated, and in processing
        // state. All process pointers reference preallocated buffers valid for
        // this call and the host provides exclusive access to the instance.
        let status = unsafe {
            let callback = (*self.plugin).process.ok_or_else(|| {
                format!(
                    "CLAP plugin '{}' has no process callback",
                    self.metadata.name
                )
            })?;
            callback(self.plugin, &process)
        };
        self.pending_parameter_events.clear();
        self.automation_events.clear();
        self.midi_events.clear();
        if self.host_requests.restart.swap(false, Ordering::AcqRel) {
            return Err(format!(
                "CLAP plugin '{}' requested a host restart/graph rebuild",
                self.metadata.name
            ));
        }
        let _requested_tail_process = self.host_requests.process.swap(false, Ordering::AcqRel);
        let _requested_main_thread_callback =
            self.host_requests.callback.swap(false, Ordering::AcqRel);
        if status == CLAP_PROCESS_ERROR {
            output[..frames * output_channels].fill(0.0);
            return Err(format!(
                "CLAP plugin '{}' reported a processing error",
                self.metadata.name
            ));
        }

        for frame in 0..frames {
            for channel in 0..output_channels {
                output[frame * output_channels + channel] =
                    self.output_storage[channel * self.max_block_frames + frame];
            }
        }
        self.steady_time = self.steady_time.saturating_add(frames as i64);
        Ok(())
    }

    fn save_state(&self) -> Result<Option<Vec<u8>>, String> {
        // SAFETY: Extension lookup and callback use the live initialized plugin
        // on the non-realtime control path. The stream context outlives `save`.
        unsafe {
            let Some(state) = plugin_extension::<clap_plugin_state>(self.plugin, CLAP_EXT_STATE)
            else {
                return Ok(None);
            };
            let Some(save) = (*state).save else {
                return Ok(None);
            };
            let mut bytes = Vec::new();
            let stream = clap_ostream {
                ctx: (&mut bytes as *mut Vec<u8>).cast(),
                write: Some(state_write),
            };
            if !save(self.plugin, &stream) {
                return Err(format!(
                    "CLAP plugin '{}' failed to save state",
                    self.metadata.name
                ));
            }
            Ok(Some(bytes))
        }
    }

    fn load_state(&mut self, state: &[u8]) -> Result<(), String> {
        if state.is_empty() {
            return Ok(());
        }
        // SAFETY: Extension lookup and callback use the live initialized plugin
        // on the non-realtime control path. The reader and byte slice outlive
        // the synchronous `load` call.
        unsafe {
            let extension = plugin_extension::<clap_plugin_state>(self.plugin, CLAP_EXT_STATE)
                .ok_or_else(|| {
                    format!(
                        "CLAP plugin '{}' has persisted state but does not expose clap.state",
                        self.metadata.name
                    )
                })?;
            let load = (*extension).load.ok_or_else(|| {
                format!(
                    "CLAP plugin '{}' has no state load callback",
                    self.metadata.name
                )
            })?;
            let mut reader = StateReader {
                bytes: state,
                offset: 0,
            };
            let stream = clap_istream {
                ctx: (&mut reader as *mut StateReader<'_>).cast(),
                read: Some(state_read),
            };
            if !load(self.plugin, &stream) {
                return Err(format!(
                    "CLAP plugin '{}' rejected persisted state",
                    self.metadata.name
                ));
            }
        }
        Ok(())
    }

    fn latency_samples(&self) -> usize {
        // SAFETY: The immutable latency extension may be queried while the
        // initialized plugin is alive. A missing extension means zero latency.
        unsafe {
            plugin_extension::<clap_plugin_latency>(self.plugin, CLAP_EXT_LATENCY)
                .and_then(|latency| (*latency).get)
                .map(|get| get(self.plugin) as usize)
                .unwrap_or(0)
        }
    }
}

impl Drop for ClapBackend {
    fn drop(&mut self) {
        // SAFETY: This is the inverse CLAP lifecycle order for the live plugin.
        // The retained library and boxed host outlive all callbacks below.
        unsafe {
            if self.processing {
                if let Some(stop) = (*self.plugin).stop_processing {
                    stop(self.plugin);
                }
                self.processing = false;
            }
            if self.active {
                if let Some(deactivate) = (*self.plugin).deactivate {
                    deactivate(self.plugin);
                }
                self.active = false;
            }
            destroy_plugin(self.plugin);
        }
        // Read the field so its lifetime relationship with the plugin remains
        // explicit even though it otherwise only exists to keep the host alive.
        let _ = &self.host;
    }
}

unsafe fn select_plugin_descriptor(
    factory: *const clap_plugin_factory,
    requested: &PluginDescriptor,
    library_path: &Path,
) -> Result<*const clap_plugin_descriptor, String> {
    // SAFETY: `factory` comes from the initialized CLAP entry and remains live
    // through the retained library.
    unsafe {
        let count = (*factory).get_plugin_count.ok_or_else(|| {
            format!(
                "CLAP factory '{}' has no descriptor count callback",
                library_path.display()
            )
        })?(factory);
        let get = (*factory).get_plugin_descriptor.ok_or_else(|| {
            format!(
                "CLAP factory '{}' has no descriptor callback",
                library_path.display()
            )
        })?;
        let mut only = ptr::null();
        let mut available = Vec::with_capacity(count as usize);
        for index in 0..count {
            let candidate = get(factory, index);
            if candidate.is_null() {
                continue;
            }
            only = candidate;
            let id = required_string((*candidate).id, "plugin id")?;
            let name = required_string((*candidate).name, "plugin name")?;
            let synthetic_name_match = requested
                .id
                .strip_prefix("clap.")
                .is_some_and(|synthetic| synthetic == name);
            if id == requested.id || name == requested.name || synthetic_name_match {
                return Ok(candidate);
            }
            available.push(format!("{id} ({name})"));
        }
        if count == 1 && !only.is_null() {
            return Ok(only);
        }
        Err(format!(
            "CLAP bundle '{}' does not contain requested plugin '{}'/'{}'; available: {}",
            library_path.display(),
            requested.id,
            requested.name,
            if available.is_empty() {
                "<none>".to_string()
            } else {
                available.join(", ")
            }
        ))
    }
}

unsafe fn descriptor_metadata(
    descriptor: *const clap_plugin_descriptor,
) -> Result<NativePluginMetadata, String> {
    // SAFETY: Caller provides a non-null descriptor owned by the live factory.
    unsafe {
        Ok(NativePluginMetadata {
            id: required_string((*descriptor).id, "plugin id")?,
            name: required_string((*descriptor).name, "plugin name")?,
            vendor: optional_string((*descriptor).vendor),
            version: optional_string((*descriptor).version),
            input_channels: 0,
            output_channels: 0,
        })
    }
}

unsafe fn query_audio_channels(
    plugin: *const clap_plugin,
    requested: &PluginDescriptor,
    metadata: &NativePluginMetadata,
) -> Result<(usize, usize), String> {
    // SAFETY: Plugin is initialized and extension data is plugin-owned.
    unsafe {
        let ports = plugin_extension::<clap_plugin_audio_ports>(plugin, CLAP_EXT_AUDIO_PORTS)
            .ok_or_else(|| {
                format!(
                    "CLAP plugin '{}' does not expose required clap.audio-ports",
                    metadata.name
                )
            })?;
        let count = (*ports).count.ok_or_else(|| {
            format!(
                "CLAP plugin '{}' audio-ports extension has no count callback",
                metadata.name
            )
        })?;
        let input_count = count(plugin, true);
        let output_count = count(plugin, false);
        if input_count > 1 || output_count > 1 {
            return Err(format!(
                "CLAP plugin '{}' exposes {input_count} input and {output_count} output buses; SOTF currently supports one main bus per direction",
                metadata.name
            ));
        }
        let input_channels = port_channels(ports, plugin, true, input_count, metadata)?;
        let output_channels = port_channels(ports, plugin, false, output_count, metadata)?;
        if output_channels == 0 {
            return Err(format!(
                "CLAP plugin '{}' has no audio output",
                metadata.name
            ));
        }
        if requested.is_instrument != (input_channels == 0) {
            return Err(format!(
                "CLAP plugin '{}' descriptor instrument flag conflicts with its {} input channels",
                metadata.name, input_channels
            ));
        }
        Ok((input_channels, output_channels))
    }
}

unsafe fn query_parameters(
    plugin: *const clap_plugin,
    metadata: &NativePluginMetadata,
) -> Result<(Vec<Parameter>, Vec<ClapParameterBinding>), String> {
    // SAFETY: Plugin is initialized and extension data is plugin-owned.
    unsafe {
        let Some(params) = plugin_extension::<clap_plugin_params>(plugin, CLAP_EXT_PARAMS) else {
            return Ok((Vec::new(), Vec::new()));
        };
        let count = (*params).count.ok_or_else(|| {
            format!(
                "CLAP plugin '{}' params extension has no count callback",
                metadata.name
            )
        })?(plugin);
        if count > MAX_EXPOSED_PARAMETERS {
            return Err(format!(
                "CLAP plugin '{}' exposes {count} parameters, exceeding the host limit {MAX_EXPOSED_PARAMETERS}",
                metadata.name
            ));
        }
        let get_info = (*params).get_info.ok_or_else(|| {
            format!(
                "CLAP plugin '{}' params extension has no get_info callback",
                metadata.name
            )
        })?;
        let mut parameters = Vec::with_capacity(count as usize);
        let mut bindings = Vec::with_capacity(count as usize);
        for index in 0..count {
            let mut info = std::mem::MaybeUninit::<clap_param_info>::zeroed();
            if !get_info(plugin, index, info.as_mut_ptr()) {
                return Err(format!(
                    "CLAP plugin '{}' failed to describe parameter {index}",
                    metadata.name
                ));
            }
            let info = info.assume_init();
            if info.flags & (CLAP_PARAM_IS_HIDDEN | CLAP_PARAM_IS_READONLY) != 0 {
                continue;
            }
            if !info.min_value.is_finite()
                || !info.max_value.is_finite()
                || !info.default_value.is_finite()
                || info.min_value > info.max_value
                || info.default_value < info.min_value
                || info.default_value > info.max_value
            {
                return Err(format!(
                    "CLAP plugin '{}' parameter {} has invalid range/default metadata",
                    metadata.name, info.id
                ));
            }
            let host_id = ParameterId::from(format!("clap.{}", info.id));
            let name = bounded_c_char_array(&info.name).unwrap_or_else(|| host_id.to_string());
            let group = bounded_c_char_array(&info.module).unwrap_or_else(|| "General".into());
            let is_bool = info.flags & CLAP_PARAM_IS_STEPPED != 0
                && info.min_value == 0.0
                && info.max_value == 1.0;
            let is_int = info.flags & CLAP_PARAM_IS_STEPPED != 0
                && info.min_value >= f64::from(i32::MIN)
                && info.max_value <= f64::from(i32::MAX);
            let (parameter, kind) = if is_bool {
                (
                    Parameter::new_bool(&host_id.to_string(), &name, info.default_value >= 0.5)
                        .with_group(&group),
                    ClapParameterKind::Bool,
                )
            } else if is_int {
                (
                    Parameter::new_int(
                        &host_id.to_string(),
                        &name,
                        info.default_value.round() as i32,
                        info.min_value.ceil() as i32,
                        info.max_value.floor() as i32,
                    )
                    .with_group(&group),
                    ClapParameterKind::Int,
                )
            } else {
                let min = info.min_value as f32;
                let max = info.max_value as f32;
                let default = info.default_value as f32;
                if !min.is_finite() || !max.is_finite() || !default.is_finite() {
                    return Err(format!(
                        "CLAP plugin '{}' parameter {} cannot be represented as f32",
                        metadata.name, info.id
                    ));
                }
                (
                    Parameter::new_float(&host_id.to_string(), &name, default, min, max)
                        .with_group(&group),
                    ClapParameterKind::Float,
                )
            };
            parameters.push(parameter);
            bindings.push(ClapParameterBinding {
                host_id,
                clap_id: info.id,
                cookie: info.cookie,
                kind,
            });
        }
        Ok((parameters, bindings))
    }
}

unsafe fn port_channels(
    ports: *const clap_plugin_audio_ports,
    plugin: *const clap_plugin,
    is_input: bool,
    count: u32,
    metadata: &NativePluginMetadata,
) -> Result<usize, String> {
    if count == 0 {
        return Ok(0);
    }
    // SAFETY: Caller guarantees a live audio-ports extension and count > 0.
    unsafe {
        let get = (*ports).get.ok_or_else(|| {
            format!(
                "CLAP plugin '{}' audio-ports extension has no get callback",
                metadata.name
            )
        })?;
        let mut info = std::mem::MaybeUninit::<clap_audio_port_info>::zeroed();
        if !get(plugin, 0, is_input, info.as_mut_ptr()) {
            return Err(format!(
                "CLAP plugin '{}' failed to describe its {} audio port",
                metadata.name,
                if is_input { "input" } else { "output" }
            ));
        }
        Ok(info.assume_init().channel_count as usize)
    }
}

unsafe fn plugin_extension<T>(plugin: *const clap_plugin, id: &CStr) -> Option<*const T> {
    // SAFETY: Caller guarantees a live plugin. CLAP extension pointers are
    // immutable and remain valid for the plugin lifetime.
    unsafe {
        let get = (*plugin).get_extension?;
        let extension = get(plugin, id.as_ptr()).cast::<T>();
        (!extension.is_null()).then_some(extension)
    }
}

unsafe fn destroy_plugin(plugin: *const clap_plugin) {
    if plugin.is_null() {
        return;
    }
    // SAFETY: Caller owns the plugin instance and invokes destroy at most once.
    unsafe {
        if let Some(destroy) = (*plugin).destroy {
            destroy(plugin);
        }
    }
}

unsafe fn required_string(pointer: *const c_char, field: &str) -> Result<String, String> {
    if pointer.is_null() {
        return Err(format!("CLAP descriptor has a null {field}"));
    }
    // SAFETY: CLAP descriptor strings are required to be NUL-terminated and
    // valid for the descriptor lifetime.
    Ok(unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned())
}

unsafe fn optional_string(pointer: *const c_char) -> String {
    if pointer.is_null() {
        String::new()
    } else {
        // SAFETY: Non-null optional CLAP descriptor strings are NUL-terminated.
        unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned()
    }
}

unsafe extern "C" fn host_get_extension(
    _host: *const clap_host,
    _extension_id: *const c_char,
) -> *const c_void {
    ptr::null()
}

unsafe fn host_requests(host: *const clap_host) -> Option<&'static ClapHostRequests> {
    if host.is_null() {
        return None;
    }
    // SAFETY: `host_data` points at the backend-owned request flags for the
    // entire CLAP instance lifetime.
    unsafe { ((*host).host_data as *const ClapHostRequests).as_ref() }
}

unsafe extern "C" fn host_request_restart(host: *const clap_host) {
    if let Some(requests) = unsafe { host_requests(host) } {
        requests.restart.store(true, Ordering::Release);
    }
}
unsafe extern "C" fn host_request_process(host: *const clap_host) {
    if let Some(requests) = unsafe { host_requests(host) } {
        requests.process.store(true, Ordering::Release);
    }
}
unsafe extern "C" fn host_request_callback(host: *const clap_host) {
    if let Some(requests) = unsafe { host_requests(host) } {
        requests.callback.store(true, Ordering::Release);
    }
}

unsafe extern "C" fn input_event_count(list: *const clap_input_events) -> u32 {
    if list.is_null() {
        return 0;
    }
    // SAFETY: Process creates `ctx` from a live event Vec for the duration of
    // the plugin callback.
    unsafe {
        let events = &*((*list).ctx.cast::<ClapInputEventLists>());
        events
            .parameters
            .len()
            .saturating_add(events.automation.len())
            .saturating_add(events.midi.len())
            .min(u32::MAX as usize) as u32
    }
}

unsafe extern "C" fn input_event_get(
    list: *const clap_input_events,
    index: u32,
) -> *const clap_event_header {
    if list.is_null() {
        return ptr::null();
    }
    // SAFETY: Same event-list lifetime invariant as `parameter_event_count`.
    unsafe {
        let events = &*((*list).ctx.cast::<ClapInputEventLists>());
        let index = index as usize;
        if let Some(event) = events.parameters.get(index) {
            &event.header
        } else if let Some(event) = events
            .automation
            .get(index.saturating_sub(events.parameters.len()))
        {
            &event.header
        } else {
            let parameter_count = events
                .parameters
                .len()
                .saturating_add(events.automation.len());
            events
                .midi
                .get(index.saturating_sub(parameter_count))
                .map_or(ptr::null(), |event| &event.header)
        }
    }
}

fn clap_transport(context: &crate::plugin::ProcessContext) -> clap_event_transport {
    let transport = context.transport;
    let mut flags = CLAP_TRANSPORT_HAS_TEMPO
        | CLAP_TRANSPORT_HAS_BEATS_TIMELINE
        | CLAP_TRANSPORT_HAS_SECONDS_TIMELINE
        | CLAP_TRANSPORT_HAS_TIME_SIGNATURE;
    if transport.playing {
        flags |= CLAP_TRANSPORT_IS_PLAYING;
    }
    if transport.recording {
        flags |= CLAP_TRANSPORT_IS_RECORDING;
    }
    if transport.looping {
        flags |= CLAP_TRANSPORT_IS_LOOP_ACTIVE;
    }
    let samples_to_seconds = |sample: u64| {
        (sample as f64 / f64::from(context.sample_rate) * CLAP_SECTIME_FACTOR as f64) as i64
    };
    let samples_to_beats = |sample: u64| {
        (sample as f64 / f64::from(context.sample_rate) * transport.bpm / 60.0
            * CLAP_BEATTIME_FACTOR as f64) as i64
    };
    let (loop_start_beats, loop_end_beats, loop_start_seconds, loop_end_seconds) =
        transport.loop_range.map_or((0, 0, 0, 0), |range| {
            (
                samples_to_beats(range.start_sample),
                samples_to_beats(range.end_sample),
                samples_to_seconds(range.start_sample),
                samples_to_seconds(range.end_sample),
            )
        });
    clap_event_transport {
        header: clap_event_header {
            size: std::mem::size_of::<clap_event_transport>() as u32,
            time: 0,
            space_id: CLAP_CORE_EVENT_SPACE_ID,
            type_: 0,
            flags: 0,
        },
        flags,
        song_pos_beats: (transport.ppq_position * CLAP_BEATTIME_FACTOR as f64) as i64,
        song_pos_seconds: samples_to_seconds(transport.sample_position),
        tempo: transport.bpm,
        tempo_inc: 0.0,
        loop_start_beats,
        loop_end_beats,
        loop_start_seconds,
        loop_end_seconds,
        bar_start: 0,
        bar_number: 0,
        tsig_num: u16::from(transport.time_signature.numerator),
        tsig_denom: u16::from(transport.time_signature.denominator),
    }
}

unsafe extern "C" fn discard_output_event(
    _list: *const clap_output_events,
    _event: *const clap_event_header,
) -> bool {
    false
}

unsafe extern "C" fn state_write(
    stream: *const clap_ostream,
    buffer: *const c_void,
    size: u64,
) -> i64 {
    if stream.is_null() || buffer.is_null() {
        return -1;
    }
    let Ok(size) = usize::try_from(size) else {
        return -1;
    };
    // SAFETY: `ctx` is a `Vec<u8>` for the synchronous save call and `buffer`
    // points to `size` readable bytes supplied by the plugin.
    unsafe {
        let bytes = &mut *((*stream).ctx.cast::<Vec<u8>>());
        let source = std::slice::from_raw_parts(buffer.cast::<u8>(), size);
        bytes.extend_from_slice(source);
    }
    size as i64
}

struct StateReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

fn bounded_c_char_array<const N: usize>(chars: &[c_char; N]) -> Option<String> {
    let bytes = chars.iter().map(|value| *value as u8).collect::<Vec<_>>();
    let nul = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    let value = String::from_utf8_lossy(&bytes[..nul]).trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn parameter_value_as_f64(value: &ParameterValue) -> f64 {
    match value {
        ParameterValue::Float(value) => f64::from(*value),
        ParameterValue::Int(value) => f64::from(*value),
        ParameterValue::Bool(value) => f64::from(u8::from(*value)),
        ParameterValue::String(_) => unreachable!("CLAP numeric parameter validated as string"),
    }
}

fn parameter_value_from_f64(kind: ClapParameterKind, value: f64) -> ParameterValue {
    match kind {
        ClapParameterKind::Float => ParameterValue::Float(value as f32),
        ClapParameterKind::Int => ParameterValue::Int(value.round() as i32),
        ClapParameterKind::Bool => ParameterValue::Bool(value >= 0.5),
    }
}

unsafe extern "C" fn state_read(
    stream: *const clap_istream,
    buffer: *mut c_void,
    size: u64,
) -> i64 {
    if stream.is_null() || buffer.is_null() {
        return -1;
    }
    let Ok(requested) = usize::try_from(size) else {
        return -1;
    };
    // SAFETY: `ctx` is the live `StateReader` for this synchronous load call;
    // the plugin supplied a writable buffer of `requested` bytes.
    unsafe {
        let reader = &mut *((*stream).ctx.cast::<StateReader<'_>>());
        let remaining = reader.bytes.len().saturating_sub(reader.offset);
        let count = requested.min(remaining);
        ptr::copy_nonoverlapping(
            reader.bytes.as_ptr().add(reader.offset),
            buffer.cast(),
            count,
        );
        reader.offset += count;
        count as i64
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use crate::plugin::{LoopRange, ProcessContext, TransportInfo};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_EVENT: AtomicUsize = AtomicUsize::new(0);
    static STOP_EVENT: AtomicUsize = AtomicUsize::new(usize::MAX);
    static DEACTIVATE_EVENT: AtomicUsize = AtomicUsize::new(usize::MAX);
    static DESTROY_EVENT: AtomicUsize = AtomicUsize::new(usize::MAX);

    unsafe extern "C" fn record_stop(_plugin: *const clap_plugin) {
        STOP_EVENT.store(NEXT_EVENT.fetch_add(1, Ordering::SeqCst), Ordering::SeqCst);
    }

    unsafe extern "C" fn record_deactivate(_plugin: *const clap_plugin) {
        DEACTIVATE_EVENT.store(NEXT_EVENT.fetch_add(1, Ordering::SeqCst), Ordering::SeqCst);
    }

    unsafe extern "C" fn record_destroy(_plugin: *const clap_plugin) {
        DESTROY_EVENT.store(NEXT_EVENT.fetch_add(1, Ordering::SeqCst), Ordering::SeqCst);
    }

    unsafe extern "C" fn accept_activation(
        _plugin: *const clap_plugin,
        _sample_rate: f64,
        _min_frames_count: u32,
        _max_frames_count: u32,
    ) -> bool {
        true
    }

    unsafe extern "C" fn accept_start_processing(_plugin: *const clap_plugin) -> bool {
        true
    }

    #[test]
    fn construction_guard_unwinds_processing_activation_and_instance_in_order() {
        NEXT_EVENT.store(0, Ordering::SeqCst);
        STOP_EVENT.store(usize::MAX, Ordering::SeqCst);
        DEACTIVATE_EVENT.store(usize::MAX, Ordering::SeqCst);
        DESTROY_EVENT.store(usize::MAX, Ordering::SeqCst);

        let plugin = clap_plugin {
            desc: ptr::null(),
            plugin_data: ptr::null_mut(),
            init: None,
            destroy: Some(record_destroy),
            activate: None,
            deactivate: Some(record_deactivate),
            start_processing: None,
            stop_processing: Some(record_stop),
            reset: None,
            process: None,
            get_extension: None,
            on_main_thread: None,
        };
        {
            let mut guard = ClapLifecycleGuard::new(&plugin);
            guard.active = true;
            guard.processing = true;
        }

        assert_eq!(STOP_EVENT.load(Ordering::SeqCst), 0);
        assert_eq!(DEACTIVATE_EVENT.load(Ordering::SeqCst), 1);
        assert_eq!(DESTROY_EVENT.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn lifecycle_validation_rejects_missing_deactivate_callback() {
        let plugin = clap_plugin {
            desc: ptr::null(),
            plugin_data: ptr::null_mut(),
            init: None,
            destroy: Some(record_destroy),
            activate: Some(accept_activation),
            deactivate: None,
            start_processing: Some(accept_start_processing),
            stop_processing: Some(record_stop),
            reset: None,
            process: None,
            get_extension: None,
            on_main_thread: None,
        };

        // SAFETY: The stack value is a live CLAP instance for the duration of
        // this callback-table validation.
        let error = unsafe { validate_clap_lifecycle_callbacks(&plugin, "test") }.unwrap_err();
        assert!(error.contains("no deactivate callback"), "{error}");
    }

    #[test]
    fn clap_event_list_preserves_nonzero_midi_offset_and_transport() {
        let midi = [clap_event_midi {
            header: clap_event_header {
                size: std::mem::size_of::<clap_event_midi>() as u32,
                time: 37,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_MIDI,
                flags: 0,
            },
            port_index: 0,
            data: [0x90, 60, 100],
        }];
        let lists = ClapInputEventLists {
            parameters: &[],
            automation: &[],
            midi: &midi,
        };
        let list = clap_input_events {
            ctx: (&lists as *const ClapInputEventLists).cast_mut().cast(),
            size: Some(input_event_count),
            get: Some(input_event_get),
        };
        assert_eq!(unsafe { input_event_count(&list) }, 1);
        assert_eq!(unsafe { (*input_event_get(&list, 0)).time }, 37);

        let transport = TransportInfo::at_sample(48_000, 48_000)
            .with_tempo(90.0, 48_000)
            .with_loop_range(LoopRange::new(24_000, 72_000));
        let translated = clap_transport(&ProcessContext::new(48_000, 64).with_transport(transport));
        assert_eq!(translated.tempo, 90.0);
        assert_eq!(translated.song_pos_seconds, CLAP_SECTIME_FACTOR);
        assert_ne!(translated.flags & CLAP_TRANSPORT_IS_LOOP_ACTIVE, 0);
    }

    #[test]
    fn clap_host_callbacks_publish_requests() {
        let requests = Box::<ClapHostRequests>::default();
        let host = clap_host {
            clap_version: CLAP_VERSION,
            host_data: (&*requests as *const ClapHostRequests).cast_mut().cast(),
            name: ptr::null(),
            vendor: ptr::null(),
            url: ptr::null(),
            version: ptr::null(),
            get_extension: None,
            request_restart: None,
            request_process: None,
            request_callback: None,
        };
        unsafe {
            host_request_restart(&host);
            host_request_process(&host);
            host_request_callback(&host);
        }
        assert!(requests.restart.load(Ordering::Acquire));
        assert!(requests.process.load(Ordering::Acquire));
        assert!(requests.callback.load(Ordering::Acquire));
    }

    #[test]
    fn lifecycle_validation_rejects_missing_destroy_callback() {
        let plugin = clap_plugin {
            desc: ptr::null(),
            plugin_data: ptr::null_mut(),
            init: None,
            destroy: None,
            activate: Some(accept_activation),
            deactivate: Some(record_deactivate),
            start_processing: Some(accept_start_processing),
            stop_processing: Some(record_stop),
            reset: None,
            process: None,
            get_extension: None,
            on_main_thread: None,
        };

        // SAFETY: The stack value is a live CLAP instance for the duration of
        // this callback-table validation.
        let error = unsafe { validate_clap_lifecycle_callbacks(&plugin, "test") }.unwrap_err();
        assert!(error.contains("no destroy callback"), "{error}");
    }

    #[test]
    fn lifecycle_validation_rejects_missing_stop_processing_callback() {
        let plugin = clap_plugin {
            desc: ptr::null(),
            plugin_data: ptr::null_mut(),
            init: None,
            destroy: Some(record_destroy),
            activate: Some(accept_activation),
            deactivate: Some(record_deactivate),
            start_processing: Some(accept_start_processing),
            stop_processing: None,
            reset: None,
            process: None,
            get_extension: None,
            on_main_thread: None,
        };

        // SAFETY: The stack value is a live CLAP instance for the duration of
        // this callback-table validation.
        let error = unsafe { validate_clap_lifecycle_callbacks(&plugin, "test") }.unwrap_err();
        assert!(error.contains("no stop_processing callback"), "{error}");
    }
}
