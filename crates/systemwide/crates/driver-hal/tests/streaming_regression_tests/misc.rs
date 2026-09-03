use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("driver-hal manifest should be nested four levels below repo root")
        .to_path_buf()
}

fn read_repo_file(path: &str) -> String {
    let path = repo_root().join(path);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("failed to read {path:?}: {err}"))
}

fn function_body<'a>(source: &'a str, function_name: &str) -> &'a str {
    let start = source
        .find(function_name)
        .unwrap_or_else(|| panic!("missing function {function_name}"));
    let rest = &source[start..];
    let open = rest
        .find('{')
        .unwrap_or_else(|| panic!("missing body for {function_name}"));
    let mut depth = 0usize;
    let body_start = start + open;

    for (offset, ch) in source[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[body_start..body_start + offset + 1];
                }
            }
            _ => {}
        }
    }

    panic!("unterminated body for {function_name}");
}

fn switch_case_body<'a>(source: &'a str, case_label: &str, next_case_label: &str) -> &'a str {
    let start = source
        .find(case_label)
        .unwrap_or_else(|| panic!("missing switch case {case_label}"));
    let after_start = &source[start..];
    let end = after_start
        .find(next_case_label)
        .unwrap_or_else(|| panic!("missing next switch case {next_case_label}"));
    &after_start[..end]
}

#[test]
fn decoder_retries_hal_reader_after_late_shared_memory_creation() {
    let source = format!(
        "{}\n{}",
        read_repo_file("crates/sotf-engine/src/engine/decoder_thread/decoder_state.rs"),
        read_repo_file("crates/sotf-engine/src/engine/decoder_thread/consts.rs")
    );
    let start_silent_source = function_body(&source, "fn start_silent_source");
    let process_hal_input = function_body(&source, "fn process_hal_input");

    assert!(
        start_silent_source.contains("self.try_reconnect_hal_reader(true);"),
        "driver-mode startup must force an initial HAL reader attempt"
    );
    assert!(
        process_hal_input.contains("self.try_reconnect_hal_reader(false);"),
        "driver-mode processing must retry HAL reader setup after the mmap appears"
    );
    assert!(
        source.contains("const HAL_RECONNECT_INTERVAL"),
        "HAL reconnect attempts should remain throttled"
    );
}

#[test]
fn daemon_reconfiguration_uses_negotiated_hal_format() {
    let source = read_repo_file(
        "crates/systemwide/crates/daemon/bin/sotf_daemon/pipeline_reconfigure_outcome.rs",
    );
    let body = function_body(&source, "fn reconfigure_audio_pipeline");

    assert!(
        source.contains("hal_sample_rate: u32"),
        "reconfiguration must accept the negotiated HAL sample rate by name"
    );
    assert!(
        source.contains("hal_buffer_frames: u32"),
        "reconfiguration must accept the negotiated HAL buffer size by name"
    );
    assert!(
        body.contains("AudioDaemon::start_pipeline_plan("),
        "reconfiguration must use the shared driver-format startup path"
    );
    assert!(
        body.contains("hal_sample_rate,"),
        "reconfiguration must pass the negotiated HAL sample rate to the engine"
    );
    assert!(
        body.contains("hal_buffer_frames,"),
        "reconfiguration must pass the negotiated HAL buffer size to the engine"
    );
    assert!(
        !body.contains("start_hal_playback(output_device"),
        "reconfiguration must not fall back to the 48 kHz default HAL startup path"
    );
}

#[test]
fn swift_encrypted_transport_is_disabled_on_realtime_paths() {
    let shared_memory =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SharedMemory.swift");
    let driver =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let daemon = read_repo_file("crates/systemwide/crates/daemon/bin/sotf_daemon/audio_daemon.rs");
    let write_audio = function_body(&shared_memory, "func writeAudio(");
    let read_audio = function_body(&shared_memory, "func readAudio(");
    let cipher_match = function_body(&shared_memory, "private func cipherMatchingHeader");
    let maintenance = function_body(&driver, "private func runMaintenanceTick");
    let io_body = function_body(&driver, "private func driverDoIOOperation");

    assert!(
        write_audio.contains("#if SOTF_ENABLE_ALLOCATING_REALTIME_ENCRYPTION")
            && read_audio.contains("#if SOTF_ENABLE_ALLOCATING_REALTIME_ENCRYPTION"),
        "allocating CryptoKit transport must be compiled out of default realtime paths"
    );
    assert!(
        !cipher_match.contains("checkAndReload") && !io_body.contains("checkAndReload"),
        "CoreAudio callbacks must never observe key files or reload ciphers"
    );
    assert!(
        maintenance.contains("EncryptionKeyManager.shared.checkAndReload()"),
        "key-file observation belongs on the HAL maintenance queue"
    );
    assert!(
        daemon.contains("Encrypted realtime transport is unavailable"),
        "the daemon must reject encryption instead of silently enabling a transport that drops frames"
    );
}

#[test]
fn swift_hal_callback_does_not_retry_shared_memory_initialization() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let io_body = function_body(&source, "private func driverDoIOOperation");
    let write_mix = switch_case_body(io_body, "case kIOOperation_WriteMix:", "default:");

    assert!(
        !write_mix.contains("attemptInitRetryIfNeeded"),
        "WriteMix runs on the CoreAudio IO path and must not open, mmap, or chmod files"
    );
}

#[test]
fn swift_hal_callback_reads_channel_geometry_without_locking() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let io_body = function_body(&source, "private func driverDoIOOperation");
    let snapshot = function_body(&source, "func callbackChannelCountSnapshot");
    let commit = function_body(&source, "private func commitActiveConfiguration");

    assert!(
        io_body.contains("state.callbackChannelCountSnapshot()"),
        "CoreAudio IO must read its channel geometry from the atomic callback snapshot"
    );
    assert!(
        !io_body.contains("activeConfiguration()") && !io_body.contains("configurationLock"),
        "CoreAudio IO must not acquire the control-plane configuration lock"
    );
    assert!(
        snapshot.contains("sotf_atomic_load_u32(&callbackChannelCount)")
            && !snapshot.contains(".lock()"),
        "the callback channel snapshot must be a direct C11 atomic load"
    );
    assert!(
        commit.contains("sotf_atomic_store_u32(&callbackChannelCount, configuration.channelCount)"),
        "committing active configuration must publish channel geometry atomically"
    );
}

#[test]
fn swift_uses_non_audio_thread_maintenance_for_late_shared_memory() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let state_body = function_body(&source, "final class DriverState");
    let start_io = function_body(&source, "private func driverStartIO");

    assert!(
        state_body.contains("DispatchSource.makeTimerSource"),
        "HAL should retry late daemon-created shared memory from a dispatch timer"
    );
    assert!(
        state_body.contains("runMaintenanceTick"),
        "maintenance timer should call a non-audio-thread tick"
    );
    assert!(
        start_io.contains("state.startMaintenanceTasks()"),
        "StartIO should ensure maintenance is running without doing retry work inline"
    );
    assert!(
        !start_io.contains("state.attemptInitRetryIfNeeded()"),
        "StartIO should not be the only retry point for late shared memory"
    );
}

#[test]
fn swift_consumes_daemon_initiated_config_requests() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let state_body = function_body(&source, "final class DriverState");

    assert!(
        state_body.contains("sharedAudio.configChanged()")
            && state_body.contains("sharedAudio.configSource() == 2"),
        "Swift HAL must poll daemon-initiated config changes"
    );
    assert!(
        state_body.contains("getRequestedSampleRate()")
            && state_body.contains("getRequestedBufferFrames()"),
        "Swift HAL must read requested daemon config values"
    );
    assert!(
        state_body.contains("acknowledgeConfigChange("),
        "Swift HAL must acknowledge daemon config requests"
    );
    assert!(
        state_body.contains("notifyPropertyChanged(objectID: kDeviceObjectID, selector: kSelector_NominalSampleRate)")
            && state_body.contains("notifyPropertyChanged(objectID: kDeviceObjectID, selector: kSelector_BufferFrameSize)"),
        "Swift HAL must notify CoreAudio when daemon config changes device format"
    );
}

#[test]
fn swift_daemon_config_requests_go_through_coreaudio_reconfiguration() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let handler = function_body(&source, "private func handleDaemonConfigRequestIfNeeded");
    let requester = function_body(&source, "private func requestDaemonConfigChange");
    let performer = function_body(
        &source,
        "private func driverPerformDeviceConfigurationChange",
    );

    assert!(
        handler.contains("requestDaemonConfigChange("),
        "daemon-initiated format changes must be handed to CoreAudio before mutating HAL state"
    );
    assert!(
        !handler.contains("bufferFrameSize = requestedFrames")
            && !handler.contains("sampleRate = Float64(requestedRate)"),
        "maintenance polling must not mutate active IO format directly"
    );
    assert!(
        requester.contains("RequestDeviceConfigurationChange("),
        "HAL must ask CoreAudio to quiesce IO before applying daemon-requested format changes"
    );
    assert!(
        performer.contains("performPendingDaemonConfigChange()"),
        "daemon-requested config must be applied from PerformDeviceConfigurationChange"
    );
}

#[test]
fn swift_property_notifications_are_gated_by_coreaudio_object_lifecycle() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let state_body = function_body(&source, "final class DriverState");
    let notify = function_body(&source, "private func notifyPropertyChanged");
    let create = function_body(&source, "private func driverCreateDevice");
    let destroy = function_body(&source, "private func driverDestroyDevice");
    let has_property = function_body(&source, "private func driverHasProperty");
    let get_size = function_body(&source, "private func driverGetPropertyDataSize");
    let get_data = function_body(&source, "private func driverGetPropertyData(");

    assert!(
        state_body.contains("deviceObjectCreated")
            && state_body.contains("observedStreamObjects")
            && state_body.contains("canNotifyPropertyChange"),
        "HAL must track which CoreAudio objects are valid before notifying property changes"
    );
    assert!(
        notify.contains("canNotifyPropertyChange(objectID: objectID)")
            && notify.contains("Skipping PropertiesChanged"),
        "PropertiesChanged must be skipped for invalid or undiscovered objects"
    );
    assert!(
        create.contains("markDeviceCreated(kDeviceObjectID)")
            && destroy.contains("markDeviceDestroyed(deviceObjectID)"),
        "CreateDevice/DestroyDevice must update notification object lifecycle"
    );
    assert!(
        has_property.contains("noteObjectAccess(objectID)")
            && get_size.contains("noteObjectAccess(objectID)")
            && get_data.contains("noteObjectAccess(objectID)"),
        "stream objects should become notifiable only after CoreAudio probes them"
    );
}

#[test]
fn swift_reports_legal_zero_time_stamp_period() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let timing = read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/Timing.swift");

    let period_line = source
        .lines()
        .find(|line| line.contains("private let kZeroTimeStampPeriod"))
        .expect("missing kZeroTimeStampPeriod");
    let period_value = period_line
        .split('=')
        .nth(1)
        .expect("period line should contain '='")
        .split("//")
        .next()
        .expect("period value should precede comment")
        .trim()
        .replace('_', "")
        .parse::<u32>()
        .expect("zero timestamp period should be a u32 literal");

    assert!(
        period_value >= 10_923,
        "kAudioDevicePropertyZeroTimeStampPeriod must be at least 10923 frames"
    );

    let get_data = function_body(&source, "private func driverGetPropertyData(");
    let zero_period_case = switch_case_body(
        get_data,
        "case kSelector_ZeroTimePeriod:",
        "case kSelector_BufferSizeRange:",
    );
    let get_zero = function_body(&source, "private func driverGetZeroTimeStamp");

    assert!(
        zero_period_case.contains("kZeroTimeStampPeriod"),
        "the HAL property must report the fixed legal zero timestamp period"
    );
    assert!(
        !zero_period_case.contains("state.bufferFrameSize"),
        "zero timestamp period must not track the small IO buffer size"
    );
    assert!(
        get_zero.contains("getZeroTimeStamp(period: kZeroTimeStampPeriod)"),
        "GetZeroTimeStamp must align to the reported zero timestamp period"
    );
    assert!(
        timing.contains("func getZeroTimeStamp(period: UInt32)"),
        "DriverClock should accept the timestamp period explicitly"
    );
}

#[test]
fn swift_ioproc_logging_is_absent_by_default() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let get_zero = function_body(&source, "private func driverGetZeroTimeStamp");
    let io_body = function_body(&source, "private func driverDoIOOperation");

    for forbidden in [
        "struct ZeroTimeLogger",
        "struct DoIOLogger",
        "struct DiagCounter",
        "struct EngineReadyTracker",
        "static var",
    ] {
        assert!(
            !get_zero.contains(forbidden) && !io_body.contains(forbidden),
            "CoreAudio IO callbacks must not mutate unsynchronized static state: {forbidden}"
        );
    }

    assert!(
        get_zero.contains("sotf_atomic_fetch_add_u64"),
        "GetZeroTimeStamp logging counters must use atomic state"
    );
    assert!(!io_body.contains("FIRST CALL"));
    assert!(
        source.contains("#if SOTF_AUDIO_TRACE")
            && !source.contains("let shouldLogDiag = false")
            && io_body.matches("#if SOTF_AUDIO_TRACE").count() >= 2,
        "callback diagnostics must only exist in SOTF_AUDIO_TRACE sections"
    );
}

#[test]
fn swift_driver_clock_state_is_lock_protected() {
    let timing = read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/Timing.swift");
    let clock = function_body(&timing, "final class DriverClock");

    assert!(
        clock.contains("private let lock = NSLock()"),
        "DriverClock needs a lock because IOProc and control threads access the same fields"
    );
    for function_name in [
        "func start(sampleRate:",
        "func stop()",
        "func setSampleRate",
        "func getZeroTimeStamp",
        "func getCurrentSampleTime()",
        "func getSeed()",
    ] {
        let body = function_body(clock, function_name);
        assert!(
            body.contains("lock.lock()") && body.contains("defer { lock.unlock() }"),
            "{function_name} must hold the DriverClock lock while reading or mutating clock state"
        );
    }
    assert!(
        clock.contains("getCurrentSampleTimeLocked"),
        "sample-time math should stay inside the locked section without recursively locking"
    );
}

#[test]
fn swift_shared_memory_is_open_only_for_restricted_hal_process() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SharedMemory.swift");

    assert!(
        source.contains("Darwin.open(currentPath, O_RDWR)"),
        "HAL should open the daemon-owned shared-memory file without creating it"
    );
    assert!(
        !source.contains("O_CREAT"),
        "HAL must not create the shared-memory file from coreaudiod"
    );
    assert!(
        !source.contains("ftruncate("),
        "HAL must not resize the shared-memory file from coreaudiod"
    );
    assert!(
        !source.contains("chmod("),
        "HAL must not mutate shared-memory permissions from coreaudiod"
    );
    assert!(
        !source.contains("createDirectory"),
        "HAL must not create arbitrary /tmp directories from coreaudiod"
    );
}

#[test]
fn swift_shared_memory_protocol_matches_rust_v6_configuring_handshake() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SharedMemory.swift");

    assert!(
        source.contains("private let kSharedMemoryVersion: UInt32 = 6"),
        "Swift shared-memory protocol version must match Rust v6"
    );
    assert!(
        source.contains("var configuring: UInt32"),
        "Swift SharedAudioHeader must include the Rust v5 configuring field"
    );
    assert!(source.contains("var configuringAck: UInt32"));
    assert!(source.contains("var requestedChannelCount: UInt32"));
    assert!(
        source.contains("var keyFingerprint: UInt64"),
        "Swift SharedAudioHeader must mirror Rust's aligned AtomicU64 key_fingerprint field"
    );
    assert!(
        source.contains("headerFingerprint >> shift"),
        "Swift must compare the UInt64 fingerprint using Rust's canonical big-endian byte order"
    );
    assert!(
        source.contains("atomicLoad(&header.pointee.configuring) & kConfiguringReconfigure")
            && source.contains("tryAcquireIOCommit"),
        "HAL write/config paths must observe the daemon configuring bitset"
    );
}

#[test]
fn swift_read_input_does_not_consume_capture_ring() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let io_body = function_body(&source, "private func driverDoIOOperation");
    let read_input = switch_case_body(
        io_body,
        "case kIOOperation_ReadInput:",
        "case kIOOperation_WriteMix:",
    );

    assert!(
        !read_input.contains("sharedAudio.readAudio"),
        "ReadInput must not consume the WriteMix capture ring before the daemon reads it"
    );
    assert!(
        read_input.contains("loopbackEnabled") && read_input.contains("silence"),
        "ReadInput should be limited to loopback or silence until separate output IPC exists"
    );
}

#[test]
fn swift_tracks_io_clients_by_client_id() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let state_body = function_body(&source, "final class DriverState");
    let start_io = function_body(&source, "private func driverStartIO");
    let stop_io = function_body(&source, "private func driverStopIO");
    let remove_client = function_body(&source, "private func driverRemoveDeviceClient");

    assert!(
        state_body.contains("activeIOClients = Set<UInt32>()"),
        "HAL must track active IO clients by clientID, not only by a global counter"
    );
    assert!(
        start_io.contains("state.startIOClient(clientID)")
            && !start_io.contains("incrementIOClientCount"),
        "StartIO should insert the CoreAudio clientID exactly once"
    );
    assert!(
        stop_io.contains("state.stopIOClient(clientID)")
            && !stop_io.contains("decrementIOClientCount"),
        "StopIO should remove the CoreAudio clientID and ignore duplicate stops"
    );
    assert!(
        remove_client.contains("state.removeIOClient(info.mClientID)"),
        "RemoveDeviceClient should clear any stale active IO state for that client"
    );
}

#[test]
fn swift_supports_hal_listener_bookkeeping_properties() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let has_property = function_body(&source, "private func driverHasProperty");
    let settable = function_body(&source, "private func driverIsPropertySettable");
    let set_data = function_body(&source, "private func driverSetPropertyData");

    assert!(
        source.contains("kSelector_Creator")
            && source.contains("kSelector_ListenerAdded")
            && source.contains("kSelector_ListenerRemoved"),
        "HAL should declare inherited AudioObject creator/listener bookkeeping selectors"
    );
    assert!(
        has_property.contains("kSelector_ListenerAdded")
            && has_property.contains("kSelector_ListenerRemoved"),
        "HAL must report listener add/remove properties as inherited common properties"
    );
    assert!(
        settable.contains("kSelector_ListenerAdded")
            && settable.contains("kSelector_ListenerRemoved"),
        "HAL shell notifies listener changes through SetPropertyData"
    );
    assert!(
        set_data.contains("case kSelector_ListenerAdded, kSelector_ListenerRemoved:"),
        "listener add/remove notifications should be accepted as no-op SetPropertyData calls"
    );
}

#[test]
fn swift_probe_logging_is_gated_off_by_default() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let has_property = function_body(&source, "private func driverHasProperty");
    let get_data = function_body(&source, "private func driverGetPropertyData(");
    let will_do = function_body(&source, "private func driverWillDoIOOperation");
    let io_body = function_body(&source, "private func driverDoIOOperation");

    assert!(
        source.contains("private let kEnableVerboseHALProbeLogging = false"),
        "HAL probe logging should be disabled by default in coreaudiod"
    );
    assert!(
        has_property.contains("halDebugLog(\"[PROBE]")
            && get_data.contains("halDebugLog(\"[PROBE]")
            && !has_property.contains("halLog(\"[PROBE]")
            && !get_data.contains("halLog(\"[PROBE]"),
        "property probe logs must be gated behind debug logging"
    );
    assert!(
        will_do.contains("halDebugLog(\"WillDoIOOperation"),
        "WillDoIOOperation is queried during stream setup and should not always log"
    );
    assert!(
        io_body.contains("kEnableVerboseHALProbeLogging && (diagCount % 200) == 0")
            && !io_body.contains("TEMP DEBUG"),
        "WriteMix diagnostics should be gated off the audio callback hot path"
    );
}

#[test]
fn swift_query_interface_does_not_leak_iunknown_refs() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let query_interface = function_body(&source, "private func queryInterface");
    let release = function_body(&source, "private func release");
    let iunknown_start = query_interface
        .find("uuidBytesEqual(iid, kIUnknownUUIDBytes)")
        .expect("QueryInterface must handle IUnknown explicitly");
    let driver_start = query_interface
        .find("uuidBytesEqual(iid, kAudioServerPlugInDriverInterfaceUUIDBytes)")
        .expect("QueryInterface must handle the CoreAudio driver interface explicitly");
    let iunknown_branch = &query_interface[iunknown_start..driver_start];

    assert!(
        iunknown_branch.contains("ppv.pointee = self_"),
        "IUnknown queries should return the existing factory interface"
    );
    assert!(
        !iunknown_branch.contains("addRef")
            && !iunknown_branch.contains("OSAtomicIncrement32(&gRefCount)"),
        "IUnknown queries on the factory interface must not bump gRefCount"
    );
    assert!(
        query_interface.contains("if driverInterface != self_")
            && query_interface.contains("_ = addRef(driverInterface)"),
        "QueryInterface should AddRef only when returning a different interface pointer"
    );
    assert!(
        query_interface.contains("return kHRESULTNoInterface"),
        "unsupported interface IDs must not be accepted or retained"
    );
    assert!(
        release.contains("if newCount < 0") && release.contains("OSAtomicIncrement32(&gRefCount)"),
        "Release must not leave gRefCount negative after an unmatched release"
    );
}

#[test]
fn swift_write_mix_falls_back_to_secondary_buffer() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let io_body = function_body(&source, "private func driverDoIOOperation");
    let write_mix = switch_case_body(io_body, "case kIOOperation_WriteMix:", "default:");

    assert!(
        source.contains("private func peakMagnitude("),
        "WriteMix should cheaply detect when CoreAudio put audio in the secondary buffer"
    );
    assert!(
        write_mix.contains("ioSecondaryBuffer")
            && write_mix.contains("selectedFloatBuffer")
            && write_mix.contains("selectedSecondaryBuffer"),
        "WriteMix should select between main and secondary CoreAudio buffers"
    );
    assert!(
        write_mix.contains("sharedAudio.writeAudio(selectedFloatBuffer")
            && write_mix.contains("outputBuffer.writeInterleaved(selectedFloatBuffer"),
        "HAL should forward the selected CoreAudio buffer to loopback and shared memory"
    );
}

#[test]
fn swift_interleaved_loopback_publishes_once_per_channel_block() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/RingBuffer.swift");
    let write = function_body(&source, "func writeInterleaved");
    let read = function_body(&source, "func readInterleaved");

    assert!(!write.contains("writeStrided") && !read.contains("readStrided"));
    assert!(write.contains("memcpy(") && read.contains("memcpy("));
    assert!(write.contains("sotf_atomic_store_u64(&writePosition"));
    assert!(read.contains("sotf_atomic_store_u64(&readPosition"));
    assert!(
        !write.contains("count: 1") && !read.contains("count: 1"),
        "interleaved loopback must not perform one ring operation per sample"
    );
}

#[test]
fn swift_loopback_ring_uses_c11_atomic_cursors() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/RingBuffer.swift");
    let ring = function_body(&source, "final class AudioRingBuffer");

    assert!(ring.contains("sotf_atomic_load_u64(&writePosition)"));
    assert!(ring.contains("sotf_atomic_load_u64(&readPosition)"));
    assert!(ring.contains("sotf_atomic_store_u64(&writePosition"));
    assert!(ring.contains("sotf_atomic_store_u64(&readPosition"));
    assert!(!ring.contains("OSMemoryBarrier()"));
    assert!(!ring.contains("writePosition +=") && !ring.contains("readPosition +="));
}

#[test]
fn swift_maintenance_uses_mapping_generation_handoff_for_active_clients() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let state = function_body(&source, "final class DriverState");
    let handoff = function_body(&source, "func handoffStaleSharedAudioMappingIfNeeded");
    let io_body = function_body(&source, "private func driverDoIOOperation");

    assert!(
        state.contains("private let sharedAudioSlots = [SharedAudioBuffer(), SharedAudioBuffer()]")
            && handoff.contains("sotf_atomic_store_u32(&activeSharedAudioSlot")
            && handoff.contains("sharedAudioReaderCount(slot: replacementSlot) == 0"),
        "maintenance must prepare and atomically publish an inactive mmap generation"
    );
    assert!(
        !handoff.contains("activeIOClients") && !handoff.contains("isEmpty"),
        "long-lived CoreAudio clients must not block stale-mmap replacement"
    );
    assert!(
        io_body.contains("state.acquireSharedAudioForCallback()")
            && io_body.contains("state.finishSharedAudioCallback"),
        "callbacks must pin their mmap generation until the IO operation finishes"
    );
    let swift_tests =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/Tests.swift");
    let restart_test = function_body(
        &swift_tests,
        "static func testGeometryCacheSurvivesDaemonRestart",
    );
    assert!(restart_test.contains("state.startIOClient(activeClientID)"));
    assert!(restart_test.contains("state.ioClientCount == 1"));
    assert!(restart_test.contains("state.ioClientCount == 0"));
    assert!(
        restart_test
            .matches("handoffStaleSharedAudioMappingIfNeeded")
            .count()
            >= 2
    );
}

#[test]
fn configbar_runs_launchctl_on_serial_lifecycle_queue() {
    let source = read_repo_file("crates/systemwide/crates/daemon/configbar/src/ConfigBar.swift");
    let manager = function_body(&source, "class DaemonManager");
    let kickstart = function_body(&source, "private func kickstartAgent");
    let launch = function_body(&source, "private func launchDaemonInBackground");
    let launchctl = function_body(&source, "private func runLaunchctl");
    let restart = function_body(&source, "func restartDaemon");

    assert!(manager.contains("org.spinorama.sotf.configbar.daemon-lifecycle"));
    assert!(kickstart.contains("lifecycleQueue.async"));
    assert!(kickstart.contains("DispatchQueue.main.async"));
    assert!(launch.contains("lifecycleQueue.async"));
    assert!(restart.contains("lifecycleQueue.async"));
    assert!(restart.contains("lifecycleQueue.asyncAfter"));
    assert!(launchctl.contains("process.waitUntilExit()"));
    assert!(
        !kickstart.contains("DispatchQueue.main.sync"),
        "launchctl completion may return to AppKit, but AppKit must never wait on lifecycle work"
    );
}

#[test]
fn swift_hal_supports_daemon_requested_channel_counts_up_to_32() {
    let hal_source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SotFHALDriver.swift");
    let shm_source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SharedMemory.swift");
    let config_handler = function_body(
        &hal_source,
        "private func handleDaemonConfigRequestIfNeeded",
    );
    let apply_config = function_body(&hal_source, "func performPendingDaemonConfigChange");
    let initialize = function_body(&shm_source, "func initialize");

    assert!(
        hal_source.contains("private let kMaxChannelCount: UInt32 = 32"),
        "Swift HAL should allow channel configurations up to 32"
    );
    assert!(
        config_handler.contains("let requestedChannels = sharedAudio.getRequestedChannelCount()")
            && config_handler
                .contains("requestedChannels >= 1 && requestedChannels <= kMaxChannelCount"),
        "daemon-initiated config validation should include channel count"
    );
    assert!(
        apply_config.contains("commitActiveConfiguration(configuration)")
            && apply_config.contains("channelCount: pending.channelCount")
            && apply_config.contains("notifyConfigurationPropertiesChanged()")
            && hal_source.contains("kSelector_StreamConfig"),
        "applying daemon config should commit channel geometry and notify CoreAudio"
    );
    assert!(
        shm_source.contains("func getRequestedChannelCount()")
            && shm_source
                .contains("atomicStore(&header.pointee.requestedChannelCount, channelCount)"),
        "shared-memory config negotiation should carry pending channel geometry"
    );
    assert!(
        initialize.contains("memorySize = Int(statBuf.st_size)")
            && initialize.contains("closeSharedMemory()"),
        "Swift shared memory should map the daemon-sized capacity so later channel growth can fit"
    );
}

#[test]
fn swift_shared_memory_drops_old_capture_when_ring_is_full() {
    let source =
        read_repo_file("crates/systemwide/crates/driver-hal/swift/Sources/SharedMemory.swift");
    let write_audio = function_body(&source, "func writeAudio");
    let write_raw = function_body(&source, "private func writeRawBytes");

    assert!(
        write_audio.contains("samplesToWrite")
            && write_audio.contains("samplesToDrop")
            && write_audio.contains("atomicStore(&header.pointee.readPosition, adjustedReadPos)"),
        "unencrypted live capture writes must drop oldest samples before publishing current audio"
    );
    assert!(
        write_audio.contains("sourceOffset")
            && !write_audio.contains("if toWrite <= 0 { return 0 }"),
        "oversized live capture writes should keep the newest complete frames instead of returning 0"
    );
    assert!(
        write_raw.contains("floatCount > audioCapacity")
            && write_raw.contains("floatCount > available")
            && write_raw.contains("atomicStore(&header.pointee.readPosition, writePos)"),
        "encrypted capture writes must drop old records at record boundaries when the ring is full"
    );
}
