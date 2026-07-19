use super::native_backend::{NativeExternalPluginBackend, NativePluginMetadata};
use super::plugin_descriptor::{PluginDescriptor, resolve_dynamic_library_path};
use crate::parameters::{Parameter, ParameterId, ParameterValue};
use libloading::Library;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};
use vst3_sys::base::{
    IBStream, IPluginBase, IPluginFactory, PClassInfo, kIBSeekCur, kIBSeekEnd, kIBSeekSet,
    kInvalidArgument, kResultFalse, kResultOk, tresult,
};
use vst3_sys::utils::{SharedVstPtr, StaticVstPtr, VstPtr};
use vst3_sys::vst::{
    AudioBusBuffers, BusDirections, BusInfo, IAudioProcessor, IComponent, IEditController,
    IHostApplication, IParamValueQueue, IParameterChanges, IoModes, K_SAMPLE32, MediaTypes,
    ParameterFlags, ParameterInfo, ProcessData, ProcessModes, ProcessSetup, SpeakerArrangement,
};
use vst3_sys::{ComInterface, IID, VST3};

const MAX_FRAMES_PER_BLOCK: usize = 65_536;
const MAX_FACTORY_CLASSES: i32 = 16_384;
const MAX_PARAMETERS: i32 = 65_536;

#[VST3(implements(IHostApplication))]
struct Vst3HostApplication {}

impl Vst3HostApplication {
    fn new() -> Box<Self> {
        Self::allocate()
    }
}

impl IHostApplication for Vst3HostApplication {
    unsafe fn get_name(&self, name: *mut u16) -> tresult {
        if name.is_null() {
            return vst3_sys::base::kInvalidArgument;
        }
        let encoded = "SOTF".encode_utf16().collect::<Vec<_>>();
        // SAFETY: VST3 defines `String128` output storage for this callback.
        unsafe {
            ptr::write_bytes(name, 0, 128);
            ptr::copy_nonoverlapping(encoded.as_ptr(), name, encoded.len());
        }
        kResultOk
    }

    unsafe fn create_instance(
        &self,
        _cid: *const IID,
        _iid: *const IID,
        object: *mut *mut c_void,
    ) -> tresult {
        if !object.is_null() {
            // SAFETY: The caller supplied the standard VST3 out pointer.
            unsafe { *object = ptr::null_mut() };
        }
        vst3_sys::base::kNoInterface
    }
}

#[VST3(implements(IBStream))]
struct Vst3MemoryStream {
    bytes: Rc<RefCell<Vec<u8>>>,
    cursor: Rc<Cell<usize>>,
    writable: bool,
}

impl Vst3MemoryStream {
    fn new(initial: &[u8], writable: bool) -> (Box<Self>, Rc<RefCell<Vec<u8>>>) {
        let bytes = Rc::new(RefCell::new(initial.to_vec()));
        let cursor = Rc::new(Cell::new(0));
        (Self::allocate(Rc::clone(&bytes), cursor, writable), bytes)
    }
}

impl IBStream for Vst3MemoryStream {
    unsafe fn read(
        &self,
        buffer: *mut c_void,
        num_bytes: i32,
        num_bytes_read: *mut i32,
    ) -> tresult {
        let Ok(requested) = usize::try_from(num_bytes) else {
            return kInvalidArgument;
        };
        if requested != 0 && buffer.is_null() {
            return kInvalidArgument;
        }
        let bytes = self.bytes.borrow();
        let cursor = self.cursor.get();
        let count = requested.min(bytes.len().saturating_sub(cursor));
        if count != 0 {
            // SAFETY: The plugin provided writable storage for `requested`
            // bytes and `count` is bounded by both buffers.
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr().add(cursor), buffer.cast(), count);
            }
        }
        self.cursor.set(cursor + count);
        if !num_bytes_read.is_null() {
            // SAFETY: This is the optional VST3 result out pointer.
            unsafe { *num_bytes_read = count as i32 };
        }
        kResultOk
    }

    unsafe fn write(
        &self,
        buffer: *const c_void,
        num_bytes: i32,
        num_bytes_written: *mut i32,
    ) -> tresult {
        let Ok(count) = usize::try_from(num_bytes) else {
            return kInvalidArgument;
        };
        if !self.writable || (count != 0 && buffer.is_null()) {
            return kResultFalse;
        }
        let cursor = self.cursor.get();
        let Some(end) = cursor.checked_add(count) else {
            return kInvalidArgument;
        };
        let mut bytes = self.bytes.borrow_mut();
        if end > bytes.len() {
            bytes.resize(end, 0);
        }
        if count != 0 {
            // SAFETY: The plugin provided readable storage for `count` bytes
            // and the destination was resized to hold the complete write.
            unsafe {
                ptr::copy_nonoverlapping(buffer.cast(), bytes.as_mut_ptr().add(cursor), count);
            }
        }
        self.cursor.set(end);
        if !num_bytes_written.is_null() {
            // SAFETY: This is the optional VST3 result out pointer.
            unsafe { *num_bytes_written = count as i32 };
        }
        kResultOk
    }

    unsafe fn seek(&self, pos: i64, mode: i32, result: *mut i64) -> tresult {
        let base = if mode == kIBSeekSet {
            0_i128
        } else if mode == kIBSeekCur {
            self.cursor.get() as i128
        } else if mode == kIBSeekEnd {
            self.bytes.borrow().len() as i128
        } else {
            return kInvalidArgument;
        };
        let target = base + i128::from(pos);
        let Ok(target) = usize::try_from(target) else {
            return kInvalidArgument;
        };
        self.cursor.set(target);
        if !result.is_null() {
            // SAFETY: This is the optional VST3 result out pointer.
            unsafe { *result = target as i64 };
        }
        kResultOk
    }

    unsafe fn tell(&self, pos: *mut i64) -> tresult {
        if pos.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: The caller provided the required VST3 result out pointer.
        unsafe { *pos = self.cursor.get() as i64 };
        kResultOk
    }
}

#[VST3(implements(IParamValueQueue))]
struct Vst3ParamValueQueue {
    id: u32,
    value: Rc<Cell<Option<f64>>>,
}

impl IParamValueQueue for Vst3ParamValueQueue {
    unsafe fn get_parameter_id(&self) -> u32 {
        self.id
    }

    unsafe fn get_point_count(&self) -> i32 {
        i32::from(self.value.get().is_some())
    }

    unsafe fn get_point(&self, index: i32, sample_offset: *mut i32, value: *mut f64) -> tresult {
        let Some(current) = self.value.get() else {
            return kInvalidArgument;
        };
        if index != 0 || sample_offset.is_null() || value.is_null() {
            return kInvalidArgument;
        }
        // SAFETY: Both required VST3 out pointers were validated above.
        unsafe {
            *sample_offset = 0;
            *value = current;
        }
        kResultOk
    }

    unsafe fn add_point(&self, _sample_offset: i32, _value: f64, _index: *mut i32) -> tresult {
        kResultFalse
    }
}

#[VST3(implements(IParameterChanges))]
struct Vst3ParameterChanges {
    queues: Vec<VstPtr<dyn IParamValueQueue>>,
    values: Vec<Rc<Cell<Option<f64>>>>,
}

impl IParameterChanges for Vst3ParameterChanges {
    unsafe fn get_parameter_count(&self) -> i32 {
        self.values
            .iter()
            .filter(|value| value.get().is_some())
            .count() as i32
    }

    unsafe fn get_parameter_data(&self, index: i32) -> StaticVstPtr<dyn IParamValueQueue> {
        let Ok(index) = usize::try_from(index) else {
            // SAFETY: Null is the ABI sentinel for an invalid queue index.
            return unsafe { null_static_vst_ptr() };
        };
        self.queues
            .iter()
            .zip(&self.values)
            .filter(|(_, value)| value.get().is_some())
            .nth(index)
            .map_or_else(
                || {
                    // SAFETY: Same null sentinel as above.
                    unsafe { null_static_vst_ptr() }
                },
                |(queue, _)| {
                    // SAFETY: This object owns the queue for the complete
                    // synchronous process call.
                    unsafe { static_vst_ptr(queue) }
                },
            )
    }

    unsafe fn add_parameter_data(
        &self,
        _id: *const u32,
        _index: *mut i32,
    ) -> StaticVstPtr<dyn IParamValueQueue> {
        // SAFETY: Input changes are immutable from the plugin's perspective.
        unsafe { null_static_vst_ptr() }
    }
}

struct Vst3Library {
    _library: Library,
    get_factory: unsafe extern "system" fn() -> *mut c_void,
}

// SAFETY: The function pointer belongs to `_library`, which is retained in the
// process-wide registry and never unloaded while it can be called.
unsafe impl Send for Vst3Library {}
// SAFETY: Factory creation is confined to serialized backend construction.
unsafe impl Sync for Vst3Library {}

fn library_registry() -> &'static Mutex<HashMap<PathBuf, Arc<Vst3Library>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<PathBuf, Arc<Vst3Library>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

impl Vst3Library {
    fn load(path: &Path) -> Result<Arc<Self>, String> {
        let path = path.canonicalize().map_err(|error| {
            format!(
                "failed to canonicalize VST3 library '{}': {error}",
                path.display()
            )
        })?;
        let mut registry = library_registry()
            .lock()
            .map_err(|_| "VST3 library registry mutex is poisoned".to_string())?;
        if let Some(library) = registry.get(&path) {
            return Ok(Arc::clone(library));
        }

        // SAFETY: The validated canonical library is retained for the process
        // lifetime, and all entry symbols are checked before invocation.
        let library = unsafe { Library::new(&path) }.map_err(|error| {
            format!(
                "failed to load VST3 plugin library '{}': {error}",
                path.display()
            )
        })?;
        initialize_platform_module(&library, &path)?;
        // SAFETY: `GetPluginFactory` is the required VST3 factory symbol and
        // the copied function pointer cannot outlive the retained library.
        let get_factory = unsafe {
            *library
                .get::<unsafe extern "system" fn() -> *mut c_void>(b"GetPluginFactory\0")
                .map_err(|error| {
                    format!(
                        "VST3 plugin '{}' is missing required symbol 'GetPluginFactory': {error}",
                        path.display()
                    )
                })?
        };
        let loaded = Arc::new(Self {
            _library: library,
            get_factory,
        });
        registry.insert(path, Arc::clone(&loaded));
        Ok(loaded)
    }
}

#[derive(Clone, Copy)]
enum Vst3ParameterKind {
    Float,
    Integer,
    Boolean,
}

struct Vst3ParameterBinding {
    host_id: ParameterId,
    vst3_id: u32,
    kind: Vst3ParameterKind,
    pending: Rc<Cell<Option<f64>>>,
}

pub(super) struct Vst3Backend {
    _library: Arc<Vst3Library>,
    _host: VstPtr<dyn IHostApplication>,
    component: VstPtr<dyn IComponent>,
    processor: VstPtr<dyn IAudioProcessor>,
    controller: Option<VstPtr<dyn IEditController>>,
    separate_controller: bool,
    parameters: Vec<Parameter>,
    parameter_bindings: Vec<Vst3ParameterBinding>,
    parameter_changes: VstPtr<dyn IParameterChanges>,
    metadata: NativePluginMetadata,
    input_storage: Vec<f32>,
    output_storage: Vec<f32>,
    input_ptrs: Vec<*mut f32>,
    output_ptrs: Vec<*mut f32>,
    active: bool,
    processing: bool,
}

// SAFETY: The backend is exclusively accessed through `&mut`, and VST3's
// processing contract permits the active component to move to one audio
// thread after all setup calls complete.
unsafe impl Send for Vst3Backend {}

struct Vst3ComponentLifecycleGuard<'a> {
    component: &'a VstPtr<dyn IComponent>,
    processor: &'a VstPtr<dyn IAudioProcessor>,
    initialized: bool,
    active: bool,
    processing: bool,
    armed: bool,
}

impl<'a> Vst3ComponentLifecycleGuard<'a> {
    fn new(
        component: &'a VstPtr<dyn IComponent>,
        processor: &'a VstPtr<dyn IAudioProcessor>,
    ) -> Self {
        Self {
            component,
            processor,
            initialized: false,
            active: false,
            processing: false,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for Vst3ComponentLifecycleGuard<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // SAFETY: The guard borrows live COM interfaces and records successful
        // lifecycle transitions until `Vst3Backend` assumes ownership.
        unsafe {
            unwind_vst3_component_lifecycle(
                self.processing,
                self.active,
                self.initialized,
                || {
                    let _ = self.processor.set_processing(0);
                },
                || {
                    let _ = self.component.set_active(0);
                },
                || {
                    let _ = self.component.terminate();
                },
            );
        }
    }
}

fn unwind_vst3_component_lifecycle(
    processing: bool,
    active: bool,
    initialized: bool,
    mut stop_processing: impl FnMut(),
    mut deactivate: impl FnMut(),
    mut terminate: impl FnMut(),
) {
    if processing {
        stop_processing();
    }
    if active {
        deactivate();
    }
    if initialized {
        terminate();
    }
}

struct Vst3BackendConstructionGuard<'a> {
    component: &'a VstPtr<dyn IComponent>,
    processor: &'a VstPtr<dyn IAudioProcessor>,
    controller: Option<&'a VstPtr<dyn IEditController>>,
    initialized: bool,
    active: bool,
    processing: bool,
    separate_controller: bool,
    armed: bool,
}

#[derive(Clone, Copy)]
struct Vst3BackendConstructionState {
    initialized: bool,
    active: bool,
    processing: bool,
    separate_controller: bool,
}

impl<'a> Vst3BackendConstructionGuard<'a> {
    fn new(
        component: &'a VstPtr<dyn IComponent>,
        processor: &'a VstPtr<dyn IAudioProcessor>,
        controller: Option<&'a VstPtr<dyn IEditController>>,
        component_lifecycle: &Vst3ComponentLifecycleGuard<'_>,
        separate_controller: bool,
    ) -> Self {
        Self {
            component,
            processor,
            controller,
            initialized: component_lifecycle.initialized,
            active: component_lifecycle.active,
            processing: component_lifecycle.processing,
            separate_controller,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for Vst3BackendConstructionGuard<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // SAFETY: The guard borrows every live COM interface created during
        // construction. Its flags record successful lifecycle transitions,
        // and the combined unwind preserves the VST3 teardown order even when
        // a separate edit controller was initialized.
        unsafe {
            unwind_vst3_backend_construction(
                Vst3BackendConstructionState {
                    initialized: self.initialized,
                    active: self.active,
                    processing: self.processing,
                    separate_controller: self.separate_controller,
                },
                || {
                    let _ = self.processor.set_processing(0);
                },
                || {
                    let _ = self.component.set_active(0);
                },
                || {
                    if let Some(controller) = self.controller {
                        let _ = controller.terminate();
                    }
                },
                || {
                    let _ = self.component.terminate();
                },
            );
        }
    }
}

fn unwind_vst3_backend_construction(
    state: Vst3BackendConstructionState,
    mut stop_processing: impl FnMut(),
    mut deactivate: impl FnMut(),
    mut terminate_controller: impl FnMut(),
    mut terminate_component: impl FnMut(),
) {
    if state.processing {
        stop_processing();
    }
    if state.active {
        deactivate();
    }
    if state.separate_controller {
        terminate_controller();
    }
    if state.initialized {
        terminate_component();
    }
}

impl Vst3Backend {
    pub(super) fn load(descriptor: &PluginDescriptor, sample_rate: u32) -> Result<Self, String> {
        let library_path = resolve_dynamic_library_path(descriptor)?;
        let library = Vst3Library::load(&library_path)?;

        // SAFETY: The factory entry belongs to the retained initialized module.
        let factory = unsafe {
            let raw = (library.get_factory)();
            VstPtr::<dyn IPluginFactory>::owned(raw.cast()).ok_or_else(|| {
                format!(
                    "VST3 plugin '{}' returned a null factory",
                    library_path.display()
                )
            })?
        };
        // SAFETY: Factory calls use initialized plugin-owned class metadata.
        let (class_id, mut metadata) =
            unsafe { select_audio_class(&factory, descriptor, &library_path)? };
        // SAFETY: The generated COM object begins with its IHostApplication
        // interface and ownership is transferred to `VstPtr`.
        let host = unsafe {
            let raw = Box::into_raw(Vst3HostApplication::new());
            VstPtr::<dyn IHostApplication>::owned(raw.cast())
                .ok_or_else(|| "failed to allocate VST3 host application".to_string())?
        };
        // SAFETY: Factory creates the requested IComponent and transfers its
        // initial reference to the host through the out pointer.
        let component = unsafe {
            let mut raw = ptr::null_mut();
            ensure_ok(
                factory.create_instance(&class_id, &<dyn IComponent>::IID, &mut raw),
                &metadata.name,
                "create component",
            )?;
            VstPtr::<dyn IComponent>::owned(raw.cast()).ok_or_else(|| {
                format!(
                    "VST3 factory returned a null component for '{}'",
                    metadata.name
                )
            })?
        };
        let processor = component.cast::<dyn IAudioProcessor>().ok_or_else(|| {
            format!(
                "VST3 class '{}' does not implement IAudioProcessor",
                metadata.name
            )
        })?;

        let mut component_lifecycle = Vst3ComponentLifecycleGuard::new(&component, &processor);
        // SAFETY: All calls below follow VST3's component lifecycle. The guard
        // records completed transitions and unwinds every later error path.
        let (input_channels, output_channels) = unsafe {
            initialize_component(
                &component,
                &processor,
                &host,
                descriptor,
                sample_rate,
                &mut component_lifecycle,
            )?
        };
        metadata.input_channels = input_channels;
        metadata.output_channels = output_channels;
        // SAFETY: Controller creation uses the live factory and initialized
        // component. A separate controller receives its own initialization.
        let (controller, separate_controller) =
            unsafe { create_edit_controller(&factory, &component, &host, &metadata.name)? };
        let mut construction_lifecycle = Vst3BackendConstructionGuard::new(
            &component,
            &processor,
            controller.as_ref(),
            &component_lifecycle,
            separate_controller,
        );
        component_lifecycle.disarm();
        drop(component_lifecycle);
        let (parameters, parameter_bindings) = match controller.as_ref() {
            Some(controller) => {
                // SAFETY: Parameter metadata queries are valid after controller
                // initialization and do not retain caller-owned pointers.
                unsafe { collect_parameters(controller, &metadata.name)? }
            }
            None => (Vec::new(), Vec::new()),
        };
        let parameter_changes = create_parameter_changes(&parameter_bindings)?;
        construction_lifecycle.disarm();
        drop(construction_lifecycle);
        let mut backend = Self {
            _library: library,
            _host: host,
            component,
            processor,
            controller,
            separate_controller,
            parameters,
            parameter_bindings,
            parameter_changes,
            metadata,
            input_storage: vec![0.0; input_channels.saturating_mul(MAX_FRAMES_PER_BLOCK)],
            output_storage: vec![0.0; output_channels.saturating_mul(MAX_FRAMES_PER_BLOCK)],
            input_ptrs: Vec::with_capacity(input_channels),
            output_ptrs: Vec::with_capacity(output_channels),
            active: true,
            processing: true,
        };
        backend.rebuild_channel_pointers();
        Ok(backend)
    }

    fn rebuild_channel_pointers(&mut self) {
        self.input_ptrs.clear();
        for channel in 0..self.metadata.input_channels {
            // SAFETY: Each pointer targets a disjoint channel in fixed storage.
            self.input_ptrs.push(unsafe {
                self.input_storage
                    .as_mut_ptr()
                    .add(channel * MAX_FRAMES_PER_BLOCK)
            });
        }
        self.output_ptrs.clear();
        for channel in 0..self.metadata.output_channels {
            // SAFETY: Each pointer targets a disjoint channel in fixed storage.
            self.output_ptrs.push(unsafe {
                self.output_storage
                    .as_mut_ptr()
                    .add(channel * MAX_FRAMES_PER_BLOCK)
            });
        }
    }

    fn suspend_for_state_load(&mut self) -> Result<(), String> {
        // SAFETY: The backend exclusively owns the active component and uses
        // the required reverse processing lifecycle before changing state.
        unsafe {
            if self.processing {
                ensure_ok(
                    self.processor.set_processing(0),
                    &self.metadata.name,
                    "stop processing for state restore",
                )?;
                self.processing = false;
            }
            if self.active {
                ensure_ok(
                    self.component.set_active(0),
                    &self.metadata.name,
                    "deactivate for state restore",
                )?;
                self.active = false;
            }
        }
        Ok(())
    }

    fn resume_after_state_load(&mut self) -> Result<(), String> {
        // SAFETY: Setup remains valid and this resumes the standard VST3
        // lifecycle after a non-realtime state operation.
        unsafe {
            if !self.active {
                ensure_ok(
                    self.component.set_active(1),
                    &self.metadata.name,
                    "reactivate after state restore",
                )?;
                self.active = true;
            }
            if !self.processing {
                ensure_ok(
                    self.processor.set_processing(1),
                    &self.metadata.name,
                    "restart processing after state restore",
                )?;
                self.processing = true;
            }
        }
        Ok(())
    }
}

impl NativeExternalPluginBackend for Vst3Backend {
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
            .ok_or_else(|| format!("VST3 parameter '{id}' is not exposed"))?;
        let plain = parameter_value_to_plain(value, binding.kind)
            .ok_or_else(|| format!("VST3 parameter '{id}' received incompatible value {value}"))?;
        let controller = self.controller.as_ref().ok_or_else(|| {
            format!(
                "VST3 plugin '{}' has no edit controller",
                self.metadata.name
            )
        })?;
        // SAFETY: The initialized controller owns the conversion and parameter
        // value; the normalized value is delivered to the processor on its
        // next process block through `IParameterChanges`.
        let normalized = unsafe {
            controller
                .plain_param_to_normalized(binding.vst3_id, plain)
                .clamp(0.0, 1.0)
        };
        if !normalized.is_finite() {
            return Err(format!(
                "VST3 plugin '{}' produced a non-finite normalized value for parameter '{id}'",
                self.metadata.name
            ));
        }
        // SAFETY: Controller is live and normalized value is finite/in range.
        unsafe {
            ensure_ok(
                controller.set_param_normalized(binding.vst3_id, normalized),
                &self.metadata.name,
                "set controller parameter",
            )?;
        }
        binding.pending.set(Some(normalized));
        Ok(())
    }

    fn get_parameter(&self, id: &ParameterId) -> Option<ParameterValue> {
        let binding = self
            .parameter_bindings
            .iter()
            .find(|binding| &binding.host_id == id)?;
        let controller = self.controller.as_ref()?;
        // SAFETY: Parameter queries and conversions are valid on the initialized
        // controller and do not retain host pointers.
        let normalized = binding
            .pending
            .get()
            .unwrap_or_else(|| unsafe { controller.get_param_normalized(binding.vst3_id) });
        let plain = unsafe { controller.normalized_param_to_plain(binding.vst3_id, normalized) };
        plain_to_parameter_value(plain, binding.kind)
    }

    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        frames: usize,
        input_channels: usize,
        output_channels: usize,
    ) -> Result<(), String> {
        if frames > MAX_FRAMES_PER_BLOCK {
            return Err(format!(
                "VST3 plugin '{}' received {frames} frames, exceeding its configured maximum {MAX_FRAMES_PER_BLOCK}",
                self.metadata.name
            ));
        }
        if input_channels != self.metadata.input_channels
            || output_channels != self.metadata.output_channels
        {
            return Err(format!(
                "VST3 plugin '{}' channel contract changed from {}→{} to {input_channels}→{output_channels} without rebuild",
                self.metadata.name, self.metadata.input_channels, self.metadata.output_channels
            ));
        }
        for frame in 0..frames {
            for channel in 0..input_channels {
                self.input_storage[channel * MAX_FRAMES_PER_BLOCK + frame] =
                    input[frame * input_channels + channel];
            }
        }
        for channel in 0..output_channels {
            self.output_storage
                [channel * MAX_FRAMES_PER_BLOCK..channel * MAX_FRAMES_PER_BLOCK + frames]
                .fill(0.0);
        }

        let mut input_bus = AudioBusBuffers {
            num_channels: input_channels as i32,
            silence_flags: 0,
            buffers: self.input_ptrs.as_mut_ptr().cast(),
        };
        let mut output_bus = AudioBusBuffers {
            num_channels: output_channels as i32,
            silence_flags: 0,
            buffers: self.output_ptrs.as_mut_ptr().cast(),
        };
        let has_parameter_changes = self
            .parameter_bindings
            .iter()
            .any(|binding| binding.pending.get().is_some());
        let mut data = ProcessData {
            process_mode: ProcessModes::kRealtime as i32,
            symbolic_sample_size: K_SAMPLE32,
            num_samples: frames as i32,
            num_inputs: i32::from(input_channels != 0),
            num_outputs: i32::from(output_channels != 0),
            inputs: if input_channels == 0 {
                ptr::null_mut()
            } else {
                &mut input_bus
            },
            outputs: if output_channels == 0 {
                ptr::null_mut()
            } else {
                &mut output_bus
            },
            // SAFETY: These VST3 ABI fields are nullable interface pointers;
            // vst3-sys models them as transparent raw-pointer wrappers.
            input_param_changes: if has_parameter_changes {
                // SAFETY: The backend owns this preallocated changes object for
                // the complete process call.
                unsafe { static_vst_ptr(&self.parameter_changes) }
            } else {
                // SAFETY: Null is the VST3 sentinel for no parameter changes.
                unsafe { null_static_vst_ptr() }
            },
            output_param_changes: unsafe { null_static_vst_ptr() },
            input_events: unsafe { null_static_vst_ptr() },
            output_events: unsafe { null_static_vst_ptr() },
            context: ptr::null_mut(),
        };
        // SAFETY: Component is active and processing, buffers are preallocated
        // and valid for `frames`, and this backend has exclusive access.
        let process_result = unsafe {
            ensure_ok(
                self.processor.process(&mut data),
                &self.metadata.name,
                "process audio",
            )
        };
        for binding in &self.parameter_bindings {
            binding.pending.set(None);
        }
        process_result?;
        for frame in 0..frames {
            for channel in 0..output_channels {
                output[frame * output_channels + channel] =
                    self.output_storage[channel * MAX_FRAMES_PER_BLOCK + frame];
            }
        }
        Ok(())
    }

    fn save_state(&self) -> Result<Option<Vec<u8>>, String> {
        let (stream, bytes) = Vst3MemoryStream::new(&[], true);
        // SAFETY: Ownership of the generated IBStream object is transferred to
        // `VstPtr`, and the component only borrows it for this synchronous call.
        let stream = unsafe {
            VstPtr::<dyn IBStream>::owned(Box::into_raw(stream).cast()).ok_or_else(|| {
                format!(
                    "failed to allocate state stream for '{}'",
                    self.metadata.name
                )
            })?
        };
        // SAFETY: The component is live, and `shared_vst_ptr` preserves the
        // stream interface pointer for the duration of the synchronous call.
        unsafe {
            ensure_ok(
                self.component.get_state(shared_vst_ptr(&stream)),
                &self.metadata.name,
                "save component state",
            )?;
        }
        drop(stream);
        let state = bytes.borrow().clone();
        Ok(Some(state))
    }

    fn load_state(&mut self, state: &[u8]) -> Result<(), String> {
        self.suspend_for_state_load()?;
        let load_result = (|| {
            let (stream, _bytes) = Vst3MemoryStream::new(state, false);
            // SAFETY: Ownership of the generated IBStream object is transferred
            // to `VstPtr` for the synchronous component state call.
            let stream = unsafe {
                VstPtr::<dyn IBStream>::owned(Box::into_raw(stream).cast()).ok_or_else(|| {
                    format!(
                        "failed to allocate restore stream for '{}'",
                        self.metadata.name
                    )
                })?
            };
            // SAFETY: The inactive component and readable stream are valid for
            // the duration of this synchronous state restore.
            unsafe {
                ensure_ok(
                    self.component.set_state(shared_vst_ptr(&stream)),
                    &self.metadata.name,
                    "restore component state",
                )?;
            }
            if let Some(controller) = self.controller.as_ref() {
                let (controller_stream, _bytes) = Vst3MemoryStream::new(state, false);
                // SAFETY: This independent stream starts at offset zero for the
                // controller's component-state synchronization call.
                let controller_stream = unsafe {
                    VstPtr::<dyn IBStream>::owned(Box::into_raw(controller_stream).cast())
                        .ok_or_else(|| {
                            format!(
                                "failed to allocate controller restore stream for '{}'",
                                self.metadata.name
                            )
                        })?
                };
                // SAFETY: The initialized controller synchronously borrows the
                // same component-state bytes from its own stream.
                unsafe {
                    ensure_ok(
                        controller.set_component_state(shared_vst_ptr(&controller_stream)),
                        &self.metadata.name,
                        "synchronize controller state",
                    )?;
                }
            }
            Ok(())
        })();
        let resume_result = self.resume_after_state_load();
        match (load_result, resume_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(load), Ok(())) => Err(load),
            (Ok(()), Err(resume)) => Err(resume),
            (Err(load), Err(resume)) => {
                Err(format!("{load}; additionally failed to resume: {resume}"))
            }
        }
    }

    fn latency_samples(&self) -> usize {
        // SAFETY: Latency query is valid for the live initialized processor.
        unsafe { self.processor.get_latency_samples() as usize }
    }
}

impl Drop for Vst3Backend {
    fn drop(&mut self) {
        // SAFETY: Reverse VST3 lifecycle order for the exclusively owned live
        // component. Smart pointers release interfaces after this method.
        unsafe {
            if self.processing {
                let _ = self.processor.set_processing(0);
                self.processing = false;
            }
            if self.active {
                let _ = self.component.set_active(0);
                self.active = false;
            }
            if self.separate_controller
                && let Some(controller) = self.controller.as_ref()
            {
                let _ = controller.terminate();
            }
            let _ = self.component.terminate();
        }
    }
}

unsafe fn create_edit_controller(
    factory: &VstPtr<dyn IPluginFactory>,
    component: &VstPtr<dyn IComponent>,
    host: &VstPtr<dyn IHostApplication>,
    plugin_name: &str,
) -> Result<(Option<VstPtr<dyn IEditController>>, bool), String> {
    // SAFETY: All interfaces belong to the live initialized module and factory.
    unsafe {
        if let Some(controller) = component.cast::<dyn IEditController>() {
            return Ok((Some(controller), false));
        }

        let mut controller_id = IID { data: [0; 16] };
        if component.get_controller_class_id(&mut controller_id) != kResultOk {
            return Ok((None, false));
        }
        let mut raw = ptr::null_mut();
        ensure_ok(
            factory.create_instance(&controller_id, &<dyn IEditController>::IID, &mut raw),
            plugin_name,
            "create edit controller",
        )?;
        let controller = VstPtr::<dyn IEditController>::owned(raw.cast()).ok_or_else(|| {
            format!("VST3 factory returned a null edit controller for '{plugin_name}'")
        })?;
        ensure_ok(
            controller.initialize(host.as_ptr().cast()),
            plugin_name,
            "initialize edit controller",
        )?;
        Ok((Some(controller), true))
    }
}

unsafe fn collect_parameters(
    controller: &VstPtr<dyn IEditController>,
    plugin_name: &str,
) -> Result<(Vec<Parameter>, Vec<Vst3ParameterBinding>), String> {
    // SAFETY: The controller is initialized and all metadata is copied before
    // each call returns.
    unsafe {
        let count = controller.get_parameter_count();
        if !(0..=MAX_PARAMETERS).contains(&count) {
            return Err(format!(
                "VST3 plugin '{plugin_name}' reported invalid parameter count {count}"
            ));
        }
        let mut parameters = Vec::with_capacity(count as usize);
        let mut bindings = Vec::with_capacity(count as usize);
        for index in 0..count {
            let mut info = std::mem::MaybeUninit::<ParameterInfo>::zeroed();
            if controller.get_parameter_info(index, info.as_mut_ptr()) != kResultOk {
                continue;
            }
            let info = info.assume_init();
            if info.flags & ParameterFlags::kIsReadOnly as i32 != 0 {
                continue;
            }
            let host_id = format!("vst3.{}", info.id);
            let name = bounded_u16_array(&info.title);
            let name = if name.is_empty() {
                host_id.clone()
            } else {
                name
            };
            let unit = bounded_u16_array(&info.units);
            let default_normalized = info.default_normalized_value.clamp(0.0, 1.0);
            let min_plain = controller.normalized_param_to_plain(info.id, 0.0);
            let max_plain = controller.normalized_param_to_plain(info.id, 1.0);
            let default_plain = controller.normalized_param_to_plain(info.id, default_normalized);
            if !min_plain.is_finite() || !max_plain.is_finite() || !default_plain.is_finite() {
                continue;
            }
            let (min_plain, max_plain) = if min_plain <= max_plain {
                (min_plain, max_plain)
            } else {
                (max_plain, min_plain)
            };
            let kind = if info.step_count == 1 {
                Vst3ParameterKind::Boolean
            } else if info.step_count > 1
                && min_plain >= f64::from(i32::MIN)
                && max_plain <= f64::from(i32::MAX)
                && min_plain.fract().abs() < f64::EPSILON
                && max_plain.fract().abs() < f64::EPSILON
                && default_plain.fract().abs() < f64::EPSILON
            {
                Vst3ParameterKind::Integer
            } else {
                Vst3ParameterKind::Float
            };
            let mut parameter = match kind {
                Vst3ParameterKind::Float => Parameter::new_float(
                    &host_id,
                    &name,
                    default_plain as f32,
                    min_plain as f32,
                    max_plain as f32,
                ),
                Vst3ParameterKind::Integer => Parameter::new_int(
                    &host_id,
                    &name,
                    default_plain.round() as i32,
                    min_plain.round() as i32,
                    max_plain.round() as i32,
                ),
                Vst3ParameterKind::Boolean => {
                    Parameter::new_bool(&host_id, &name, default_normalized >= 0.5)
                }
            };
            parameter.unit = unit;
            let parameter_id = parameter.id.clone();
            parameters.push(parameter);
            bindings.push(Vst3ParameterBinding {
                host_id: parameter_id,
                vst3_id: info.id,
                kind,
                pending: Rc::new(Cell::new(None)),
            });
        }
        Ok((parameters, bindings))
    }
}

fn create_parameter_changes(
    bindings: &[Vst3ParameterBinding],
) -> Result<VstPtr<dyn IParameterChanges>, String> {
    let mut queues = Vec::with_capacity(bindings.len());
    let mut values = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let queue = Vst3ParamValueQueue::allocate(binding.vst3_id, Rc::clone(&binding.pending));
        // SAFETY: Ownership of each generated queue object is transferred to
        // the smart pointer retained by the changes object.
        let queue = unsafe {
            VstPtr::<dyn IParamValueQueue>::owned(Box::into_raw(queue).cast())
                .ok_or_else(|| "failed to allocate VST3 parameter queue".to_string())?
        };
        queues.push(queue);
        values.push(Rc::clone(&binding.pending));
    }
    let changes = Vst3ParameterChanges::allocate(queues, values);
    // SAFETY: Ownership of the generated changes object is transferred to the
    // backend smart pointer.
    unsafe {
        VstPtr::<dyn IParameterChanges>::owned(Box::into_raw(changes).cast())
            .ok_or_else(|| "failed to allocate VST3 parameter changes".to_string())
    }
}

fn parameter_value_to_plain(value: &ParameterValue, kind: Vst3ParameterKind) -> Option<f64> {
    match (kind, value) {
        (Vst3ParameterKind::Float, ParameterValue::Float(value)) => Some(f64::from(*value)),
        (Vst3ParameterKind::Integer, ParameterValue::Int(value)) => Some(f64::from(*value)),
        (Vst3ParameterKind::Boolean, ParameterValue::Bool(value)) => {
            Some(f64::from(u8::from(*value)))
        }
        _ => None,
    }
}

fn plain_to_parameter_value(plain: f64, kind: Vst3ParameterKind) -> Option<ParameterValue> {
    if !plain.is_finite() {
        return None;
    }
    match kind {
        Vst3ParameterKind::Float => Some(ParameterValue::Float(plain as f32)),
        Vst3ParameterKind::Integer => Some(ParameterValue::Int(
            plain
                .round()
                .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32,
        )),
        Vst3ParameterKind::Boolean => Some(ParameterValue::Bool(plain >= 0.5)),
    }
}

unsafe fn select_audio_class(
    factory: &VstPtr<dyn IPluginFactory>,
    requested: &PluginDescriptor,
    library_path: &Path,
) -> Result<(IID, NativePluginMetadata), String> {
    // SAFETY: Factory belongs to the live initialized VST3 module.
    unsafe {
        let count = factory.count_classes();
        if !(0..=MAX_FACTORY_CLASSES).contains(&count) {
            return Err(format!(
                "VST3 plugin '{}' reported invalid class count {count}",
                library_path.display()
            ));
        }
        let mut only = None;
        let mut available = Vec::new();
        for index in 0..count {
            let mut info = std::mem::MaybeUninit::<PClassInfo>::zeroed();
            if factory.get_class_info(index, info.as_mut_ptr()) != kResultOk {
                continue;
            }
            let info = info.assume_init();
            let category = bounded_i8_array(&info.category);
            if category != "Audio Module Class" {
                continue;
            }
            let name = bounded_i8_array(&info.name);
            let id = iid_string(&info.cid);
            let metadata = NativePluginMetadata {
                id: id.clone(),
                name: name.clone(),
                vendor: requested.vendor.clone(),
                version: requested.version.clone(),
                input_channels: 0,
                output_channels: 0,
            };
            only = Some((info.cid, metadata.clone()));
            let synthetic_name_match = requested
                .id
                .strip_prefix("vst3.")
                .is_some_and(|synthetic| synthetic == name);
            if requested.id.eq_ignore_ascii_case(&id)
                || requested.name == name
                || synthetic_name_match
            {
                return Ok((info.cid, metadata));
            }
            available.push(format!("{id} ({name})"));
        }
        if available.len() == 1 {
            return only.ok_or_else(|| "VST3 factory has no audio class".to_string());
        }
        Err(format!(
            "VST3 bundle '{}' does not contain requested plugin '{}'/'{}'; available: {}",
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

unsafe fn initialize_component(
    component: &VstPtr<dyn IComponent>,
    processor: &VstPtr<dyn IAudioProcessor>,
    host: &VstPtr<dyn IHostApplication>,
    requested: &PluginDescriptor,
    sample_rate: u32,
    lifecycle: &mut Vst3ComponentLifecycleGuard<'_>,
) -> Result<(usize, usize), String> {
    // SAFETY: Caller owns all live COM interfaces and invokes the lifecycle in
    // the required order.
    unsafe {
        ensure_ok(
            component.initialize(host.as_ptr().cast()),
            &requested.name,
            "initialize component",
        )?;
        lifecycle.initialized = true;
        ensure_ok(
            component.set_io_mode(IoModes::kSimple as i32),
            &requested.name,
            "select simple I/O mode",
        )?;
        let input_channels = audio_bus_channels(component, true, requested)?;
        let output_channels = audio_bus_channels(component, false, requested)?;
        if output_channels == 0 {
            return Err(format!(
                "VST3 plugin '{}' has no audio output",
                requested.name
            ));
        }
        if requested.is_instrument != (input_channels == 0) {
            return Err(format!(
                "VST3 plugin '{}' descriptor instrument flag conflicts with its {input_channels} input channels",
                requested.name
            ));
        }
        let mut input_arrangement = speaker_arrangement(input_channels)?;
        let mut output_arrangement = speaker_arrangement(output_channels)?;
        ensure_ok(
            processor.set_bus_arrangements(
                if input_channels == 0 {
                    ptr::null_mut()
                } else {
                    &mut input_arrangement
                },
                i32::from(input_channels != 0),
                &mut output_arrangement,
                1,
            ),
            &requested.name,
            "set bus arrangements",
        )?;
        if input_channels != 0 {
            ensure_ok(
                component.activate_bus(
                    MediaTypes::kAudio as i32,
                    BusDirections::kInput as i32,
                    0,
                    1,
                ),
                &requested.name,
                "activate input bus",
            )?;
        }
        ensure_ok(
            component.activate_bus(
                MediaTypes::kAudio as i32,
                BusDirections::kOutput as i32,
                0,
                1,
            ),
            &requested.name,
            "activate output bus",
        )?;
        let setup = ProcessSetup {
            process_mode: ProcessModes::kRealtime as i32,
            symbolic_sample_size: K_SAMPLE32,
            max_samples_per_block: MAX_FRAMES_PER_BLOCK as i32,
            sample_rate: f64::from(sample_rate),
        };
        ensure_ok(
            processor.setup_processing(&setup),
            &requested.name,
            "configure processing",
        )?;
        ensure_ok(
            component.set_active(1),
            &requested.name,
            "activate component",
        )?;
        lifecycle.active = true;
        ensure_ok(
            processor.set_processing(1),
            &requested.name,
            "start processing",
        )?;
        lifecycle.processing = true;
        Ok((input_channels, output_channels))
    }
}

unsafe fn audio_bus_channels(
    component: &VstPtr<dyn IComponent>,
    input: bool,
    requested: &PluginDescriptor,
) -> Result<usize, String> {
    let direction = if input {
        BusDirections::kInput as i32
    } else {
        BusDirections::kOutput as i32
    };
    // SAFETY: Component is initialized and the requested bus metadata is
    // plugin-owned.
    unsafe {
        let count = component.get_bus_count(MediaTypes::kAudio as i32, direction);
        if !(0..=1).contains(&count) {
            return Err(format!(
                "VST3 plugin '{}' exposes {count} {} audio buses; SOTF currently supports one main bus per direction",
                requested.name,
                if input { "input" } else { "output" }
            ));
        }
        if count == 0 {
            return Ok(0);
        }
        let mut info = std::mem::MaybeUninit::<BusInfo>::zeroed();
        ensure_ok(
            component.get_bus_info(MediaTypes::kAudio as i32, direction, 0, info.as_mut_ptr()),
            &requested.name,
            if input {
                "query input bus"
            } else {
                "query output bus"
            },
        )?;
        let channels = info.assume_init().channel_count;
        usize::try_from(channels).map_err(|_| {
            format!(
                "VST3 plugin '{}' reported negative {} channel count {channels}",
                requested.name,
                if input { "input" } else { "output" }
            )
        })
    }
}

fn speaker_arrangement(channels: usize) -> Result<SpeakerArrangement, String> {
    use vst3_sys::vst::{k40Music, k51, k71_2, k71_4, k71Music, kEmpty, kMono, kStereo};
    match channels {
        0 => Ok(kEmpty),
        1 => Ok(kMono),
        2 => Ok(kStereo),
        4 => Ok(k40Music),
        6 => Ok(k51),
        8 => Ok(k71Music),
        10 => Ok(k71_2),
        12 => Ok(k71_4),
        _ => Err(format!(
            "VST3 channel layout with {channels} channels has no canonical SOTF speaker arrangement"
        )),
    }
}

fn ensure_ok(result: tresult, plugin: &str, operation: &str) -> Result<(), String> {
    if result == kResultOk {
        Ok(())
    } else {
        Err(format!(
            "VST3 plugin '{plugin}' failed to {operation} (tresult {result})"
        ))
    }
}

fn bounded_i8_array<const N: usize>(chars: &[i8; N]) -> String {
    let nul = chars.iter().position(|value| *value == 0).unwrap_or(N);
    let bytes = chars[..nul]
        .iter()
        .map(|value| *value as u8)
        .collect::<Vec<_>>();
    String::from_utf8_lossy(&bytes).trim().to_string()
}

fn bounded_u16_array<const N: usize>(chars: &[i16; N]) -> String {
    let nul = chars.iter().position(|value| *value == 0).unwrap_or(N);
    let utf16 = chars[..nul]
        .iter()
        .map(|value| *value as u16)
        .collect::<Vec<_>>();
    String::from_utf16_lossy(&utf16).trim().to_string()
}

fn iid_string(iid: &IID) -> String {
    iid.data
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<String>()
}

unsafe fn null_static_vst_ptr<I: ComInterface + ?Sized>() -> StaticVstPtr<I> {
    // SAFETY: VST3 ABI declares these ProcessData interface fields nullable.
    // `StaticVstPtr` is `repr(transparent)` over the same raw interface pointer.
    unsafe { std::mem::transmute(ptr::null_mut::<*mut I::VTable>()) }
}

unsafe fn static_vst_ptr<I: ComInterface + ?Sized>(pointer: &VstPtr<I>) -> StaticVstPtr<I> {
    // SAFETY: `StaticVstPtr` is `repr(transparent)` over the same live interface
    // pointer and never outlives the owning `VstPtr` in this backend.
    unsafe { std::mem::transmute(pointer.as_ptr()) }
}

unsafe fn shared_vst_ptr<I: ComInterface + ?Sized>(pointer: &VstPtr<I>) -> SharedVstPtr<I> {
    // SAFETY: `SharedVstPtr` is `repr(transparent)` over the same interface
    // pointer and is only used for a synchronous borrowed ABI argument.
    unsafe { std::mem::transmute(pointer.as_ptr()) }
}

fn initialize_platform_module(library: &Library, path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: The optional VST3 bundle entry has the platform ABI and is
        // called once while the library is retained. A null bundle handle is
        // accepted by the SOTF/NIH fixture and plugins that do not need it.
        if let Ok(entry) =
            unsafe { library.get::<unsafe extern "C" fn(*mut c_void) -> bool>(b"bundleEntry\0") }
            && !unsafe { entry(ptr::null_mut()) }
        {
            return Err(format!(
                "VST3 plugin '{}' rejected bundleEntry",
                path.display()
            ));
        }
    }
    #[cfg(all(target_family = "unix", not(target_os = "macos")))]
    {
        // SAFETY: Same lifetime/ABI invariant as the macOS entry.
        if let Ok(entry) =
            unsafe { library.get::<unsafe extern "C" fn(*mut c_void) -> bool>(b"ModuleEntry\0") }
            && !unsafe { entry(ptr::null_mut()) }
        {
            return Err(format!(
                "VST3 plugin '{}' rejected ModuleEntry",
                path.display()
            ));
        }
    }
    #[cfg(target_os = "windows")]
    {
        // SAFETY: Same lifetime/ABI invariant as the macOS entry.
        if let Ok(entry) =
            unsafe { library.get::<unsafe extern "system" fn() -> bool>(b"InitDll\0") }
            && !unsafe { entry() }
        {
            return Err(format!("VST3 plugin '{}' rejected InitDll", path.display()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod lifecycle_tests {
    use super::{
        Vst3BackendConstructionState, unwind_vst3_backend_construction,
        unwind_vst3_component_lifecycle,
    };
    use std::cell::RefCell;

    #[test]
    fn construction_guard_unwinds_processing_activation_and_initialization_in_order() {
        let events = RefCell::new(Vec::new());
        unwind_vst3_component_lifecycle(
            true,
            true,
            true,
            || events.borrow_mut().push("stop-processing"),
            || events.borrow_mut().push("deactivate"),
            || events.borrow_mut().push("terminate"),
        );
        assert_eq!(
            events.into_inner(),
            vec!["stop-processing", "deactivate", "terminate"]
        );
    }

    #[test]
    fn construction_guard_only_unwinds_completed_transitions() {
        let events = RefCell::new(Vec::new());
        unwind_vst3_component_lifecycle(
            false,
            false,
            true,
            || events.borrow_mut().push("stop-processing"),
            || events.borrow_mut().push("deactivate"),
            || events.borrow_mut().push("terminate"),
        );
        assert_eq!(events.into_inner(), vec!["terminate"]);
    }

    #[test]
    fn backend_construction_guard_unwinds_full_separate_controller_order() {
        let events = RefCell::new(Vec::new());
        unwind_vst3_backend_construction(
            Vst3BackendConstructionState {
                initialized: true,
                active: true,
                processing: true,
                separate_controller: true,
            },
            || events.borrow_mut().push("stop-processing"),
            || events.borrow_mut().push("deactivate"),
            || events.borrow_mut().push("terminate-controller"),
            || events.borrow_mut().push("terminate-component"),
        );
        assert_eq!(
            events.into_inner(),
            vec![
                "stop-processing",
                "deactivate",
                "terminate-controller",
                "terminate-component",
            ]
        );
    }
}
