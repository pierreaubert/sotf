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
