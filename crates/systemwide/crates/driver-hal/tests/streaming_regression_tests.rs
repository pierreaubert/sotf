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
    let source = read_repo_file("crates/sotf-engine/src/engine/decoder_thread.rs");
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
    let source = read_repo_file("crates/systemwide/crates/daemon/bin/sotf_daemon.rs");
    let body = function_body(&source, "fn reconfigure_audio_pipeline");

    assert!(
        source.contains("hal_sample_rate: u32"),
        "reconfiguration must accept the negotiated HAL sample rate by name"
    );
    assert!(
        body.contains("start_hal_playback_with_driver_config("),
        "reconfiguration must use the explicit driver-format startup path"
    );
    assert!(
        body.contains("hal_sample_rate,"),
        "reconfiguration must pass the negotiated HAL sample rate to the engine"
    );
    assert!(
        !body.contains("start_hal_playback(output_device"),
        "reconfiguration must not fall back to the 48 kHz default HAL startup path"
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
        io_body.contains("kEnableVerboseHALProbeLogging && (DiagCounter.count % 200) == 0")
            && !io_body.contains("TEMP DEBUG"),
        "WriteMix diagnostics should be gated off the audio callback hot path"
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
        write_mix.contains("state.sharedAudio.writeAudio(selectedFloatBuffer")
            && write_mix.contains("outputBuffer.writeInterleaved(selectedFloatBuffer"),
        "HAL should forward the selected CoreAudio buffer to loopback and shared memory"
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
            && write_audio.contains("header.pointee.readPosition = adjustedReadPos"),
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
            && write_raw.contains("header.pointee.readPosition = writePos"),
        "encrypted capture writes must drop old records at record boundaries when the ring is full"
    );
}
