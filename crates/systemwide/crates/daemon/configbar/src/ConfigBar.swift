//
// SotF Systemwide Menu Bar Application
//
// A macOS menu bar app that controls the SotF Systemwide audio engine with:
// - Color-coded speaker icon (grey/green/red) based on audio activity
// - Configuration window for audio interfaces and plugin chains
// - Energy optimization (stops engine after 3s of silence)
// - Integration with src-audio daemon via Unix socket

import SwiftUI
import Cocoa
import UserNotifications
import CoreAudio

// MARK: - Audio Source Selection

/// Represents the audio input source for the daemon
enum AudioSource: String, CaseIterable, Identifiable {
    case halDriver = "SotF HAL Driver"
    case blackhole = "BlackHole"

    var id: String { rawValue }

    /// Pattern to match in device names
    var devicePattern: String {
        switch self {
        case .halDriver: return "SotF"
        case .blackhole: return "BlackHole"
        }
    }

    /// Description for the user
    var description: String {
        switch self {
        case .halDriver:
            return "Use the SotF virtual audio driver to capture system audio directly"
        case .blackhole:
            return "Use BlackHole as a virtual audio device - set it as your macOS System Output"
        }
    }

    /// Setup instructions
    var setupInstructions: String {
        switch self {
        case .halDriver:
            return "The HAL driver is automatically installed. Select it as your output device in Sound preferences."
        case .blackhole:
            return "1. Set BlackHole as your macOS System Output in Sound preferences\n2. The daemon will capture audio from BlackHole"
        }
    }
}

// MARK: - Audio Engine Client

/// Client for communicating with the sotf-daemon via Unix socket
class AudioEngineClient {
    /// Get the secure socket path (per-user directory)
    private static func getSecureSocketPath() -> String {
        // On macOS, TMPDIR is per-user and already secured
        if let tmpdir = ProcessInfo.processInfo.environment["TMPDIR"] {
            return (tmpdir as NSString).appendingPathComponent("sotf-daemon.sock")
        }
        // Fallback to UID-based path
        return "/tmp/sotf-\(getuid())/daemon.sock"
    }

    /// Legacy socket path for backwards compatibility
    private static let legacySocketPath = "/tmp/autoeq_audio.sock"

    static func socketPaths() -> [String] {
        let securePath = getSecureSocketPath()
        if securePath == legacySocketPath {
            return [securePath]
        }
        return [securePath, legacySocketPath]
    }

    /// Try secure path first, then legacy path
    private var socketPath: String {
        for path in Self.socketPaths() {
            if FileManager.default.fileExists(atPath: path) {
                return path
            }
        }
        return Self.legacySocketPath
    }

    private var socketFD: Int32 = -1

    enum AudioState: String {
        case idle = "Idle"
        case playing = "Playing"
        case recording = "Recording"
        case paused = "Paused"
        case stopped = "Stopped"
        case error = "Error"

        var iconColor: NSColor {
            switch self {
            case .idle, .stopped, .paused:
                return .systemGray
            case .playing:
                return .systemGreen
            case .recording:
                return .systemRed
            case .error:
                return .systemOrange
            }
        }
    }

    struct Response: Codable {
        let success: Bool
        let data: [String: AnyCodable]?
        let error: String?
    }

    struct AnyCodable: Codable {
        let value: Any

        init(from decoder: Decoder) throws {
            let container = try decoder.singleValueContainer()
            if let intVal = try? container.decode(Int.self) {
                value = intVal
            } else if let doubleVal = try? container.decode(Double.self) {
                value = doubleVal
            } else if let stringVal = try? container.decode(String.self) {
                value = stringVal
            } else if let boolVal = try? container.decode(Bool.self) {
                value = boolVal
            } else if let arrayVal = try? container.decode([AnyCodable].self) {
                value = arrayVal.map { $0.value }
            } else if let dictVal = try? container.decode([String: AnyCodable].self) {
                value = dictVal.mapValues { $0.value }
            } else {
                value = NSNull()
            }
        }

        func encode(to encoder: Encoder) throws {
            var container = encoder.singleValueContainer()
            if let intVal = value as? Int {
                try container.encode(intVal)
            } else if let doubleVal = value as? Double {
                try container.encode(doubleVal)
            } else if let stringVal = value as? String {
                try container.encode(stringVal)
            } else if let boolVal = value as? Bool {
                try container.encode(boolVal)
            } else if let arrayVal = value as? [Any] {
                let codableArray = arrayVal.map { AnyCodable(wrapping: $0) }
                try container.encode(codableArray)
            } else if let dictVal = value as? [String: Any] {
                let codableDict = dictVal.mapValues { AnyCodable(wrapping: $0) }
                try container.encode(codableDict)
            } else if value is NSNull {
                try container.encodeNil()
            }
        }

        /// Initialize with a value to wrap
        init(wrapping value: Any) {
            self.value = value
        }
    }

    func connect() -> Bool {
        guard FileManager.default.fileExists(atPath: socketPath) else {
            print("Socket not found at \(socketPath)")
            return false
        }

        // Close existing connection if any
        if socketFD >= 0 {
            close(socketFD)
        }

        // Create Unix domain socket
        socketFD = socket(AF_UNIX, SOCK_STREAM, 0)
        guard socketFD >= 0 else {
            print("Failed to create socket")
            return false
        }

        // Connect to daemon
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)

        withUnsafeMutableBytes(of: &addr.sun_path) { pathBuffer in
            _ = socketPath.withCString { pathCString in
                strlcpy(pathBuffer.baseAddress!.assumingMemoryBound(to: CChar.self),
                       pathCString,
                       pathBuffer.count)
            }
        }

        let connectResult = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockaddrPtr in
                Darwin.connect(socketFD, sockaddrPtr, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }

        if connectResult < 0 {
            print("Failed to connect to daemon: \(String(cString: strerror(errno)))")
            close(socketFD)
            socketFD = -1
            return false
        }

        return true
    }

    deinit {
        if socketFD >= 0 {
            close(socketFD)
        }
    }

    func sendCommand(_ command: [String: Any]) -> Response? {
        // Reconnect for each command to ensure clean state
        guard connect() else {
            return nil
        }

        defer {
            // Close connection after command
            if socketFD >= 0 {
                close(socketFD)
                socketFD = -1
            }
        }

        do {
            // Send command
            let jsonData = try JSONSerialization.data(withJSONObject: command)
            let jsonString = String(data: jsonData, encoding: .utf8)! + "\n"
            let commandBytes = [UInt8](jsonString.utf8)

            let sendResult = commandBytes.withUnsafeBufferPointer { bufferPtr in
                Darwin.send(socketFD, bufferPtr.baseAddress, commandBytes.count, 0)
            }

            guard sendResult > 0 else {
                print("Failed to send command: \(String(cString: strerror(errno)))")
                return nil
            }

            // Read response with buffered line-based parsing
            // TCP streams may fragment data, so we need to read until we find a newline
            var responseData = Data()
            var buffer = [UInt8](repeating: 0, count: 4096)
            let bufferCount = buffer.count
            let maxResponseSize = 1024 * 1024 // Plugin metadata can exceed 64KB
            let timeoutMs: useconds_t = 1000000 // Keep UI-facing calls from hanging the app

            // Set socket to non-blocking for timeout handling
            let flags = fcntl(socketFD, F_GETFL, 0)
            _ = fcntl(socketFD, F_SETFL, flags | O_NONBLOCK)

            var totalWaitTime: useconds_t = 0
            let pollInterval: useconds_t = 5000 // 5ms poll interval

            while responseData.count < maxResponseSize && totalWaitTime < timeoutMs {
                let bytesRead = buffer.withUnsafeMutableBufferPointer { bufferPtr in
                    Darwin.recv(socketFD, bufferPtr.baseAddress, bufferCount, 0)
                }

                if bytesRead > 0 {
                    responseData.append(contentsOf: buffer[0..<bytesRead])

                    // Check if we have a complete line
                    if responseData.contains(UInt8(ascii: "\n")) {
                        break
                    }
                } else if bytesRead == 0 {
                    // Connection closed by peer
                    break
                } else {
                    let err = errno
                    if err == EAGAIN || err == EWOULDBLOCK {
                        // No data available yet, wait a bit
                        usleep(pollInterval)
                        totalWaitTime += pollInterval
                    } else {
                        // Real error
                        print("Failed to read response: \(String(cString: strerror(err)))")
                        return nil
                    }
                }
            }

            if responseData.count >= maxResponseSize,
               !responseData.contains(UInt8(ascii: "\n")) {
                print("Daemon response exceeded \(maxResponseSize) bytes without a complete JSON line")
                return nil
            }

            // Restore blocking mode
            _ = fcntl(socketFD, F_SETFL, flags)

            guard !responseData.isEmpty else {
                print("Empty response from daemon (timeout or connection closed)")
                return nil
            }

            // Parse response (find JSON line)
            if let newlineIndex = responseData.firstIndex(of: UInt8(ascii: "\n")) {
                let jsonData = responseData[0..<newlineIndex]
                let response = try JSONDecoder().decode(Response.self, from: jsonData)
                return response
            } else {
                // No newline found, try parsing the whole response
                let response = try JSONDecoder().decode(Response.self, from: responseData)
                return response
            }
        } catch {
            print("Failed to send command: \(error)")
        }

        return nil
    }

    func getStatus() -> (state: AudioState, volume: Float, muted: Bool) {
        let command = ["command": "status"]

        guard let response = sendCommand(command),
              response.success,
              let data = response.data else {
            return (.idle, 1.0, false)
        }

        let stateStr = data["state"]?.value as? String ?? "Idle"
        let state = AudioState(rawValue: stateStr) ?? .idle
        let volume = (data["volume"]?.value as? Double).map { Float($0) } ?? 1.0
        let muted = data["muted"]?.value as? Bool ?? false

        return (state, volume, muted)
    }

    struct AudioDevice: Codable {
        let name: String
        let is_default: Bool
        let channels: Int?
        let sample_rate: Int?
    }

    func listDevices() -> [AudioDevice] {
        let command = ["command": "list_devices"]

        guard let response = sendCommand(command),
              response.success,
              let data = response.data,
              let devicesArray = data["devices"]?.value as? [[String: Any]] else {
            return []
        }

        // Parse device objects
        var devices: [AudioDevice] = []
        for deviceDict in devicesArray {
            if let name = deviceDict["name"] as? String {
                let isDefault = deviceDict["is_default"] as? Bool ?? false
                let channels = deviceDict["channels"] as? Int
                let sampleRate = deviceDict["sample_rate"] as? Int

                devices.append(AudioDevice(
                    name: name,
                    is_default: isDefault,
                    channels: channels,
                    sample_rate: sampleRate
                ))
            }
        }

        return devices
    }

    func setDevice(_ device: String) -> Bool {
        let command: [String: Any] = ["command": "set_device", "device": device]
        return sendCommand(command)?.success ?? false
    }

    func setVolume(_ volume: Float) -> Bool {
        let command: [String: Any] = ["command": "set_volume", "volume": volume]
        return sendCommand(command)?.success ?? false
    }

    func play() -> Bool {
        let command = ["command": "play"]
        return sendCommand(command)?.success ?? false
    }

    func pause() -> Bool {
        let command = ["command": "pause"]
        return sendCommand(command)?.success ?? false
    }

    func stop() -> Bool {
        let command = ["command": "stop"]
        return sendCommand(command)?.success ?? false
    }

    /// Loudness metering data from the daemon
    struct LoudnessData {
        var momentary: Double = -60.0
        var shortTerm: Double = -60.0
        var integrated: Double = -60.0
        var peak: Double = 0.0
        var channelPeaks: [Double] = []
        var truePeaksDbtp: [Double] = []
        var correlationLR: Double? = nil
    }

    func getLoudness() -> LoudnessData? {
        let command = ["command": "get_loudness"]

        guard let response = sendCommand(command),
              response.success,
              let data = response.data else {
            return nil
        }

        var loudness = LoudnessData()
        loudness.momentary = numberValue(data["momentary"]?.value) ?? -60.0
        loudness.shortTerm = numberValue(data["short_term"]?.value) ?? -60.0
        loudness.integrated = numberValue(data["integrated"]?.value) ?? -60.0
        loudness.peak = numberValue(data["peak"]?.value) ?? 0.0
        loudness.channelPeaks = numberArrayValue(data["channel_peaks"]?.value)
        loudness.truePeaksDbtp = numberArrayValue(data["true_peaks_dbtp"]?.value)
        loudness.correlationLR = numberValue(data["correlation_lr"]?.value)

        return loudness
    }

    // MARK: - Metering Commands

    struct MeteringData {
        var input: LoudnessData?
        var output: LoudnessData?
    }

    func getMetering() -> MeteringData? {
        let command: [String: Any] = ["command": "get_metering"]

        guard let response = sendCommand(command),
              response.success,
              let data = response.data else {
            return nil
        }

        var metering = MeteringData()

        if let inputDict = data["input"]?.value as? [String: Any] {
            metering.input = parseLoudnessDict(inputDict)
        }
        if let outputDict = data["output"]?.value as? [String: Any] {
            metering.output = parseLoudnessDict(outputDict)
        }

        return metering
    }

    private func parseLoudnessDict(_ dict: [String: Any]) -> LoudnessData {
        var loudness = LoudnessData()
        loudness.momentary = numberValue(dict["momentary"]) ?? -60.0
        loudness.shortTerm = numberValue(dict["short_term"]) ?? -60.0
        loudness.integrated = numberValue(dict["integrated"]) ?? -60.0
        loudness.peak = numberValue(dict["peak"]) ?? 0.0
        loudness.channelPeaks = numberArrayValue(dict["channel_peaks"])
        loudness.truePeaksDbtp = numberArrayValue(dict["true_peaks_dbtp"])
        loudness.correlationLR = numberValue(dict["correlation_lr"])

        return loudness
    }

    // MARK: - Plugin Management Commands

    /// Get current plugin chain from daemon (user plugins only)
    func getPlugins() -> [[String: Any]]? {
        let command: [String: Any] = ["command": "get_plugins"]

        guard let response = sendCommand(command),
              response.success,
              let data = response.data,
              let plugins = data["plugins"]?.value as? [Any] else {
            return nil
        }

        return plugins.compactMap { $0 as? [String: Any] }
    }

    /// Get available plugin types from daemon
    func getAvailablePlugins() -> [AvailablePlugin]? {
        let command: [String: Any] = ["command": "get_available_plugins"]

        guard let response = sendCommand(command),
              response.success,
              let data = response.data,
              let plugins = data["plugins"]?.value as? [Any] else {
            return nil
        }

        return plugins.compactMap { item -> AvailablePlugin? in
            guard let dict = item as? [String: Any],
                  let type_ = dict["type"] as? String,
                  let name = dict["name"] as? String,
                  let description = dict["description"] as? String,
                  let category = dict["category"] as? String,
                  let maturity = dict["maturity"] as? String else {
                return nil
            }
            let parameters = parsePluginParameterDescriptors(dict["parameters"])
            return AvailablePlugin(
                type_: type_,
                name: name,
                description: description,
                category: category,
                maturity: maturity,
                defaultParameters: dict["default_parameters"] as? [String: Any] ?? [:],
                parameters: parameters
            )
        }
    }

    private func parsePluginParameterDescriptors(_ raw: Any?) -> [PluginParameterDescriptor] {
        guard let items = raw as? [[String: Any]] else {
            return []
        }

        return items.compactMap { item in
            guard let key = item["key"] as? String,
                  let name = item["name"] as? String,
                  let type = item["type"] as? String else {
                return nil
            }

            let defaultDouble = numberValue(item["default"])

            return PluginParameterDescriptor(
                key: key,
                name: name,
                type: type,
                unit: item["unit"] as? String ?? "",
                group: item["group"] as? String ?? "General",
                doc: item["doc"] as? String ?? "",
                updateMode: item["update_mode"] as? String ?? "realtime",
                min: numberValue(item["min"]),
                max: numberValue(item["max"]),
                step: numberValue(item["step"]),
                defaultDouble: defaultDouble,
                defaultBool: item["default"] as? Bool,
                choices: item["choices"] as? [String],
                trueLabel: item["true_label"] as? String,
                falseLabel: item["false_label"] as? String
            )
        }
    }

    private func numberValue(_ raw: Any?) -> Double? {
        if let double = raw as? Double {
            return double
        }
        if let int = raw as? Int {
            return Double(int)
        }
        if let number = raw as? NSNumber {
            return number.doubleValue
        }
        if let string = raw as? String {
            return Double(string)
        }
        return nil
    }

    private func numberArrayValue(_ raw: Any?) -> [Double] {
        guard let values = raw as? [Any] else {
            return []
        }
        return values.compactMap(numberValue)
    }

    /// Add a plugin to the chain
    func addPlugin(type: String, parameters: [String: Any], index: Int?) -> Bool {
        var command: [String: Any] = [
            "command": "add_plugin",
            "plugin": [
                "plugin_type": type,
                "parameters": parameters,
            ] as [String: Any],
        ]
        if let idx = index {
            command["index"] = idx
        }
        return sendCommand(command)?.success ?? false
    }

    /// Remove a plugin by index
    func removePlugin(at index: Int) -> Bool {
        let command: [String: Any] = ["command": "remove_plugin", "index": index]
        return sendCommand(command)?.success ?? false
    }

    /// Update plugin parameters
    func updatePlugin(at index: Int, parameters: [String: Any]) -> Bool {
        let command: [String: Any] = [
            "command": "update_plugin",
            "index": index,
            "parameters": parameters,
        ]
        return sendCommand(command)?.success ?? false
    }

    /// Reorder plugins
    func reorderPlugins(order: [Int]) -> Bool {
        let command: [String: Any] = ["command": "reorder_plugins", "order": order]
        return sendCommand(command)?.success ?? false
    }

    // MARK: - Encryption Commands

    /// Encryption status from the daemon
    struct EncryptionStatusData {
        var enabled: Bool = false
        var fingerprint: String = ""
        var keyPath: String = ""
        var frameCount: UInt64 = 0
    }

    func setEncryption(enabled: Bool) -> Bool {
        let command: [String: Any] = ["command": "set_encryption", "enabled": enabled]
        return sendCommand(command)?.success ?? false
    }

    func getEncryptionStatus() -> EncryptionStatusData? {
        let command = ["command": "encryption_status"]

        guard let response = sendCommand(command),
              response.success,
              let data = response.data else {
            return nil
        }

        var status = EncryptionStatusData()
        status.enabled = data["enabled"]?.value as? Bool ?? false
        status.fingerprint = data["fingerprint"]?.value as? String ?? ""
        status.keyPath = data["key_path"]?.value as? String ?? ""
        status.frameCount = (data["frame_count"]?.value as? Int).map { UInt64($0) } ?? 0

        return status
    }

    func rotateEncryptionKey() -> Bool {
        let command = ["command": "rotate_encryption_key"]
        return sendCommand(command)?.success ?? false
    }

    // MARK: - HAL Config Commands

    /// HAL configuration data from the daemon
    struct HalConfigData {
        var sampleRate: UInt32 = 48000
        var actualSampleRate: UInt32 = 48000
        var bufferFrames: UInt32 = 512
        var actualBufferFrames: UInt32 = 512
        var channelCount: UInt32 = 2
        var active: Bool = false
        var driverReady: Bool = false
        var configStatus: UInt32 = 0  // 0=pending, 1=accepted, 2=negotiated, 3=error
        var configSource: UInt32 = 0  // 1=HAL, 2=Daemon
    }

    /// Get HAL driver configuration
    func getHalConfig() -> HalConfigData? {
        let command = ["command": "get_hal_config"]

        guard let response = sendCommand(command),
              response.success,
              let data = response.data else {
            return nil
        }

        var config = HalConfigData()
        config.sampleRate = (data["sample_rate"]?.value as? Int).map { UInt32($0) } ?? 48000
        config.actualSampleRate = (data["actual_sample_rate"]?.value as? Int).map { UInt32($0) } ?? config.sampleRate
        config.bufferFrames = (data["buffer_frames"]?.value as? Int).map { UInt32($0) } ?? 512
        config.actualBufferFrames = (data["actual_buffer_frames"]?.value as? Int).map { UInt32($0) } ?? config.bufferFrames
        config.channelCount = (data["channel_count"]?.value as? Int).map { UInt32($0) } ?? 2
        config.active = data["active"]?.value as? Bool ?? false
        config.driverReady = data["driver_ready"]?.value as? Bool ?? false
        config.configStatus = (data["config_status"]?.value as? Int).map { UInt32($0) } ?? 0
        config.configSource = (data["config_source"]?.value as? Int).map { UInt32($0) } ?? 0

        return config
    }

    /// Set HAL driver sample rate
    func setSampleRate(_ rate: UInt32) -> Bool {
        let command: [String: Any] = ["command": "set_sample_rate", "rate": rate]
        return sendCommand(command)?.success ?? false
    }

    /// Set HAL driver buffer frames
    func setBufferFrames(_ frames: UInt32) -> Bool {
        let command: [String: Any] = ["command": "set_buffer_frames", "frames": frames]
        return sendCommand(command)?.success ?? false
    }
}

// MARK: - Daemon Manager

/// Manages the sotf_daemon process lifecycle
class DaemonManager {
    private var daemonProcess: Process?
    private var watchdogTimer: Timer?
    private let daemonPath: String
    private var isShuttingDown = false

    /// Callback when daemon status changes
    var onStatusChange: ((Bool) -> Void)?

    init() {
        if let overridePath = ProcessInfo.processInfo.environment["SOTF_DAEMON_PATH"],
           FileManager.default.isExecutableFile(atPath: overridePath) {
            daemonPath = overridePath
            print("DaemonManager: Using daemon path from SOTF_DAEMON_PATH: \(daemonPath)")
            return
        }

        // Look for daemon in several locations (note: binary is named sotf-daemon with hyphen)
        let possiblePaths = [
            // In app bundle's Helpers directory
            Bundle.main.bundlePath + "/Contents/Helpers/sotf-daemon",
            // In same directory as the toolbar binary
            (Bundle.main.bundlePath as NSString).appendingPathComponent("sotf-daemon"),
            // One level up from bundle (e.g. running from within target/release/)
            (Bundle.main.bundlePath as NSString).deletingLastPathComponent + "/sotf-daemon",
            // System-wide installation
            "/usr/local/bin/sotf-daemon",
            // Development build (from project root)
            FileManager.default.currentDirectoryPath + "/target/release/sotf-daemon",
        ]

        daemonPath = possiblePaths.first { FileManager.default.isExecutableFile(atPath: $0) } ?? possiblePaths[0]
        print("DaemonManager: Using daemon path: \(daemonPath)")
    }

    /// Kill any existing sotf-daemon processes (not managed by us)
    private func killExistingDaemons() {
        // Find and kill any existing sotf-daemon or sotf_daemon processes
        let processNames = ["sotf-daemon", "sotf_daemon"]

        for processName in processNames {
            let task = Process()
            task.executableURL = URL(fileURLWithPath: "/usr/bin/pkill")
            task.arguments = ["-9", "-f", processName]

            do {
                try task.run()
                task.waitUntilExit()
                if task.terminationStatus == 0 {
                    print("DaemonManager: Killed existing \(processName) process(es)")
                    // Give the OS a moment to clean up
                    usleep(100000) // 100ms
                }
            } catch {
                // pkill failing is fine - means no matching processes
            }
        }
    }

    private func removeStaleSockets() {
        for path in AudioEngineClient.socketPaths() {
            if FileManager.default.fileExists(atPath: path) {
                try? FileManager.default.removeItem(atPath: path)
            }
        }
    }

    /// Start the daemon if not already running
    func startDaemon() {
        guard !isShuttingDown else { return }

        // Check if already running (our managed process)
        if let process = daemonProcess, process.isRunning {
            print("DaemonManager: Daemon already running (PID: \(process.processIdentifier))")
            return
        }

        // Kill any existing daemon processes not managed by us
        killExistingDaemons()
        removeStaleSockets()

        // Check if daemon exists
        guard FileManager.default.isExecutableFile(atPath: daemonPath) else {
            print("DaemonManager: Daemon not found at \(daemonPath)")
            onStatusChange?(false)
            return
        }

        print("DaemonManager: Starting daemon...")

        let process = Process()
        process.executableURL = URL(fileURLWithPath: daemonPath)
        process.arguments = []

        // Redirect output to log file
        let appSupportDir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
            .appendingPathComponent("org.spinorama.sotf")
        try? FileManager.default.createDirectory(at: appSupportDir, withIntermediateDirectories: true)
        let logPath = appSupportDir.appendingPathComponent("sotf-daemon.log").path
        FileManager.default.createFile(atPath: logPath, contents: nil)
        let logHandle = FileHandle(forWritingAtPath: logPath)
        logHandle?.seekToEndOfFile()
        process.standardOutput = logHandle
        process.standardError = logHandle
        process.environment = ProcessInfo.processInfo.environment.merging(["RUST_LOG": "info"]) { _, new in new }

        // Handle daemon termination
        process.terminationHandler = { [weak self] proc in
            DispatchQueue.main.async {
                print("DaemonManager: Daemon terminated with status \(proc.terminationStatus)")
                self?.daemonProcess = nil
                self?.onStatusChange?(false)

                // Restart if not shutting down and terminated unexpectedly
                if !(self?.isShuttingDown ?? true) && proc.terminationStatus != 0 {
                    print("DaemonManager: Restarting daemon in 2 seconds...")
                    DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
                        self?.startDaemon()
                    }
                }
            }
        }

        do {
            try process.run()
            daemonProcess = process
            print("DaemonManager: Daemon started (PID: \(process.processIdentifier))")
            onStatusChange?(true)

            // Start watchdog
            startWatchdog()
        } catch {
            print("DaemonManager: Failed to start daemon: \(error)")
            onStatusChange?(false)
        }
    }

    /// Stop the daemon
    func stopDaemon() {
        isShuttingDown = true
        stopWatchdog()

        if let process = daemonProcess, process.isRunning {
            print("DaemonManager: Stopping daemon (PID: \(process.processIdentifier))...")
            process.terminate()

            // Give it a moment to clean up
            DispatchQueue.global().asyncAfter(deadline: .now() + 1.0) {
                if process.isRunning {
                    print("DaemonManager: Force killing daemon...")
                    process.interrupt()
                }
            }
        }
        daemonProcess = nil
    }

    /// Check if daemon is running
    var isDaemonRunning: Bool {
        return daemonProcess?.isRunning ?? false
    }

    /// Start watchdog timer to monitor daemon health
    private func startWatchdog() {
        stopWatchdog()

        watchdogTimer = Timer.scheduledTimer(withTimeInterval: 5.0, repeats: true) { [weak self] _ in
            guard let self = self, !self.isShuttingDown else { return }

            // Safe check: use optional chaining to avoid force unwrap race
            let isRunning = self.daemonProcess?.isRunning ?? false
            if !isRunning {
                print("DaemonManager: Watchdog detected daemon not running, restarting...")
                self.startDaemon()
            }
        }
    }

    /// Stop watchdog timer
    private func stopWatchdog() {
        watchdogTimer?.invalidate()
        watchdogTimer = nil
    }

    deinit {
        stopDaemon()
    }
}

// MARK: - Status Bar Controller

class StatusBarController: NSObject, ObservableObject {
    private var statusItem: NSStatusItem!
    @Published var currentState: AudioEngineClient.AudioState = .idle
    @Published var showingWindow = false
    private var configWindow: NSWindow?

    private let client = AudioEngineClient()
    private var monitorTimer: Timer?
    private var statusRequestInFlight = false

    // Daemon management
    private let daemonManager = DaemonManager()
    @Published var daemonRunning = false

    override init() {
        super.init()

        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)

        if let button = statusItem.button {
            button.image = makeMenuBarIcon()
            button.imagePosition = .imageOnly
            button.toolTip = "SotF Audio Engine"
        }

        // Create menu for the status item
        let menu = NSMenu()

        let configItem = NSMenuItem(title: "Configure...", action: #selector(openConfiguration), keyEquivalent: ",")
        configItem.target = self
        menu.addItem(configItem)

        menu.addItem(NSMenuItem.separator())

        let daemonStatusItem = NSMenuItem(title: "Daemon: Starting...", action: nil, keyEquivalent: "")
        daemonStatusItem.tag = 102
        menu.addItem(daemonStatusItem)

        let statusMenuItem = NSMenuItem(title: "Status: Idle", action: nil, keyEquivalent: "")
        statusMenuItem.tag = 100  // Tag for updating later
        menu.addItem(statusMenuItem)

        menu.addItem(NSMenuItem.separator())

        // HAL Driver status
        let halStatusItem = NSMenuItem(title: "HAL Driver: " + (isHALDriverInstalled() ? "✓ Installed" : "✗ Not Installed"), action: nil, keyEquivalent: "")
        halStatusItem.tag = 101
        menu.addItem(halStatusItem)

        menu.addItem(NSMenuItem.separator())

        let quitItem = NSMenuItem(title: "Quit", action: #selector(quitApp), keyEquivalent: "q")
        quitItem.target = self
        menu.addItem(quitItem)

        statusItem.menu = menu

        // Setup daemon status callback
        daemonManager.onStatusChange = { [weak self] running in
            self?.daemonRunning = running
            self?.updateDaemonStatus()
            if running {
                // Give daemon a moment to start, then connect
                DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                    _ = self?.client.connect()
                }
            }
        }

        // Start daemon automatically
        daemonManager.startDaemon()

        // Start monitoring
        startMonitoring()

        updateIcon()

        print("StatusBarController initialized with menu")
    }

    private func updateDaemonStatus() {
        if let menu = statusItem.menu,
           let item = menu.item(withTag: 102) {
            item.title = daemonRunning ? "Daemon: ✓ Running" : "Daemon: ✗ Not Running"
        }
    }

    @objc func openConfiguration() {
        showConfigWindow()
    }

    @objc func quitApp() {
        NSApplication.shared.terminate(nil)
    }

    // MARK: - HAL Driver Management

    private let halDriverPath = "/Library/Audio/Plug-Ins/HAL/SotFHAL.driver"

    func isHALDriverInstalled() -> Bool {
        // Check for current driver or legacy driver
        return FileManager.default.fileExists(atPath: halDriverPath) ||
               FileManager.default.fileExists(atPath: "/Library/Audio/Plug-Ins/HAL/sotf.driver")
    }

    private func updateHALDriverStatus() {
        if let menu = statusItem.menu,
           let halStatusItem = menu.item(withTag: 101) {
            halStatusItem.title = "HAL Driver: " + (isHALDriverInstalled() ? "✓ Installed" : "✗ Not Installed")
        }
    }

    /// Build a vector template image so AppKit can tint it for the menu bar appearance.
    private func makeMenuBarIcon() -> NSImage {
        let iconSize = NSSize(width: 22, height: 22)
        let image = NSImage(size: iconSize, flipped: false) { rect in
            let viewBox: CGFloat = 24
            let scale = min(rect.width, rect.height) / viewBox
            let xOffset = rect.minX + (rect.width - viewBox * scale) / 2
            let yOffset = rect.minY + (rect.height - viewBox * scale) / 2

            func point(_ x: CGFloat, _ y: CGFloat) -> NSPoint {
                NSPoint(x: xOffset + x * scale, y: yOffset + (viewBox - y) * scale)
            }

            func svgRect(x: CGFloat, y: CGFloat, width: CGFloat, height: CGFloat) -> NSRect {
                NSRect(
                    x: xOffset + x * scale,
                    y: yOffset + (viewBox - y - height) * scale,
                    width: width * scale,
                    height: height * scale
                )
            }

            let strokeColor = NSColor.black
            strokeColor.setStroke()
            strokeColor.setFill()

            let strokeWidth = 2.0 * scale

            let headband = NSBezierPath()
            headband.lineWidth = strokeWidth
            headband.lineCapStyle = .round
            headband.lineJoinStyle = .round
            headband.move(to: point(3, 11.5))
            headband.curve(
                to: point(21, 11.5),
                controlPoint1: point(3.5, 3.6),
                controlPoint2: point(20.5, 3.6)
            )
            headband.stroke()

            for rectSpec in [
                (x: CGFloat(1), y: CGFloat(11.5), width: CGFloat(4), height: CGFloat(7), radius: CGFloat(2)),
                (x: CGFloat(19), y: CGFloat(11.5), width: CGFloat(4), height: CGFloat(7), radius: CGFloat(2)),
                (x: CGFloat(8), y: CGFloat(7), width: CGFloat(8), height: CGFloat(13), radius: CGFloat(1)),
            ] {
                let path = NSBezierPath(
                    roundedRect: svgRect(
                        x: rectSpec.x,
                        y: rectSpec.y,
                        width: rectSpec.width,
                        height: rectSpec.height
                    ),
                    xRadius: rectSpec.radius * scale,
                    yRadius: rectSpec.radius * scale
                )
                path.lineWidth = strokeWidth
                path.stroke()
            }

            NSBezierPath(
                ovalIn: svgRect(x: 11, y: 9, width: 2, height: 2)
            ).fill()

            let dial = NSBezierPath(
                ovalIn: svgRect(x: 10, y: 13, width: 4, height: 4)
            )
            dial.lineWidth = strokeWidth
            dial.stroke()

            return true
        }
        image.isTemplate = true
        return image
    }

    func startMonitoring() {
        monitorTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
            self?.updateStatus()
        }
    }

    func stopMonitoring() {
        monitorTimer?.invalidate()
        monitorTimer = nil
    }

    func stopDaemon() {
        daemonManager.stopDaemon()
    }

    private func updateStatus() {
        guard !statusRequestInFlight else { return }
        statusRequestInFlight = true

        DispatchQueue.global(qos: .utility).async {
            let (state, _, _) = AudioEngineClient().getStatus()

            DispatchQueue.main.async { [weak self] in
                guard let self = self else { return }
                self.statusRequestInFlight = false

                if self.currentState != state {
                    self.currentState = state
                    self.updateIcon()

                    // Update menu item
                    if let menu = self.statusItem.menu, let statusItem = menu.item(withTag: 100) {
                        statusItem.title = "Status: \(state.rawValue)"
                    }
                }
            }
        }
    }

    private func updateIcon() {
        guard let button = statusItem.button else { return }

        // For menubar template icons, don't set contentTintColor as it breaks
        // the automatic light/dark adaptation. Instead, use different symbols
        // or keep it monochrome and show status in the menu.

        // Only tint when actively playing (green) or error (red)
        switch currentState {
        case .playing:
            button.contentTintColor = .systemGreen
        case .error:
            button.contentTintColor = .systemRed
        default:
            // Let system handle the color for template images
            button.contentTintColor = nil
        }
    }

    private func showConfigWindow() {
        // If window already exists, just bring it to front
        if let existingWindow = configWindow, existingWindow.isVisible {
            existingWindow.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 800, height: 700),
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered,
            defer: false
        )

        window.title = "SotF Systemwide Configuration"
        window.center()
        window.minSize = NSSize(width: 800, height: 500)

        // IMPORTANT: Don't release window when closed, just hide it
        // This prevents the app from quitting when the window is closed
        window.isReleasedWhenClosed = false

        let contentView = ConfigurationView(
            client: client,
            onClose: { [weak window] in
                window?.close()
            }
        )

        window.contentView = NSHostingView(rootView: contentView)
        window.makeKeyAndOrderFront(nil)

        // Store reference to window
        configWindow = window
        showingWindow = true

        // Bring app to front
        NSApp.activate(ignoringOtherApps: true)
    }
}

// MARK: - Level Meter View

/// Converts dB value to a normalized position (0.0 to 1.0) with non-linear scaling
/// Emphasizes the upper range (closer to 0dB) where most action happens
func dbToPosition(_ db: Double) -> Double {
    // Non-linear mapping: -60dB = 0%, -30dB = 33%, -10dB = 66%, 0dB = 100%
    if db <= -60.0 { return 0.0 }
    if db <= -30.0 { return ((db + 60.0) / 30.0) * 0.33 }
    if db <= -10.0 { return 0.33 + ((db + 30.0) / 20.0) * 0.33 }
    return min(1.0, 0.66 + ((db + 10.0) / 10.0) * 0.34)
}

private func linearPeakToDb(_ level: Double) -> Double {
    guard level.isFinite, level > 0.00001 else {
        return -60.0
    }
    return 20.0 * log10(level)
}

private func lufsToPosition(_ lufs: Double) -> Double {
    guard lufs.isFinite else {
        return 0.0
    }
    let normalized = min(max((lufs + 60.0) / 60.0, 0.0), 1.0)
    return normalized * normalized
}

/// Single channel GPUI-style level meter bar with segmented fill and peak hold.
struct LevelMeterBar: View {
    let level: Double  // Linear peak value (0.0 to 1.0+)
    let peakHoldLevel: Double?
    let width: CGFloat

    init(level: Double, peakHoldLevel: Double? = nil, width: CGFloat = 16) {
        self.level = level
        self.peakHoldLevel = peakHoldLevel
        self.width = width
    }

    var body: some View {
        GeometryReader { geometry in
            let height = geometry.size.height
            let db = linearPeakToDb(level)
            let fillRatio = dbToPosition(db)
            let yellowThreshold = dbToPosition(-6.0)
            let redThreshold = dbToPosition(-1.0)
            let greenHeight = min(fillRatio, yellowThreshold)
            let yellowHeight = fillRatio > yellowThreshold
                ? min(fillRatio - yellowThreshold, redThreshold - yellowThreshold)
                : 0.0
            let redHeight = fillRatio > redThreshold ? fillRatio - redThreshold : 0.0

            ZStack(alignment: .bottom) {
                RoundedRectangle(cornerRadius: 2)
                    .fill(Color.black.opacity(0.38))

                VStack(spacing: 0) {
                    Spacer(minLength: 0)
                    if redHeight > 0.001 {
                        Rectangle()
                            .fill(Color(red: 0.95, green: 0.18, blue: 0.18))
                            .frame(height: CGFloat(redHeight) * height)
                    }
                    if yellowHeight > 0.001 {
                        Rectangle()
                            .fill(Color(red: 0.95, green: 0.72, blue: 0.18))
                            .frame(height: CGFloat(yellowHeight) * height)
                    }
                    if greenHeight > 0.001 {
                        Rectangle()
                            .fill(Color(red: 0.22, green: 0.78, blue: 0.34))
                            .frame(height: CGFloat(greenHeight) * height)
                    }
                }
                .clipShape(RoundedRectangle(cornerRadius: 2))

                if let peakHoldLevel,
                   peakHoldLevel > 0.00001,
                   peakHoldLevel.isFinite {
                    let peakRatio = dbToPosition(linearPeakToDb(peakHoldLevel))
                    let markerY = min(max(height - (CGFloat(peakRatio) * height), 1), height - 1)
                    Rectangle()
                        .fill(Color.white.opacity(0.95))
                        .frame(width: width, height: 2)
                        .position(x: width / 2, y: markerY)
                        .shadow(color: .black.opacity(0.65), radius: 1)
                        .zIndex(2)
                }
            }
        }
        .frame(width: width)
    }
}

struct LevelMeterScale: View {
    private let ticks: [Double] = [0, -6, -12, -18, -24, -30, -40, -50, -60]
    let width: CGFloat

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .topLeading) {
                ForEach(ticks, id: \.self) { db in
                    let y = tickY(for: db, height: geometry.size.height)
                    HStack(spacing: 2) {
                        Text(tickLabel(db))
                            .font(.system(size: 8))
                            .foregroundColor(.secondary)
                            .frame(width: max(width - 6, 12), alignment: .trailing)
                        Rectangle()
                            .fill(Color.secondary.opacity(0.45))
                            .frame(width: 4, height: 1)
                    }
                    .position(x: width / 2, y: y)
                }
            }
        }
        .frame(width: width)
    }

    private func tickY(for db: Double, height: CGFloat) -> CGFloat {
        let rawY = height - (CGFloat(dbToPosition(db)) * height)
        return min(max(rawY, 6), max(height - 6, 6))
    }

    private func tickLabel(_ db: Double) -> String {
        db == 0 ? "0" : String(Int(db))
    }
}

struct LufsMiniMeterRow: View {
    let label: String
    let value: Double

    var body: some View {
        HStack(spacing: 3) {
            Text(label)
                .font(.system(size: 9, weight: .semibold))
                .foregroundColor(.secondary)
                .frame(width: 10, alignment: .leading)

            GeometryReader { geometry in
                ZStack(alignment: .leading) {
                    RoundedRectangle(cornerRadius: 2)
                        .fill(Color.black.opacity(0.35))
                    RoundedRectangle(cornerRadius: 2)
                        .fill(Color.white.opacity(0.85))
                        .frame(width: CGFloat(lufsToPosition(value)) * geometry.size.width)
                }
            }
            .frame(height: 6)

            Text(formatLufs(value))
                .font(.system(size: 9, design: .monospaced))
                .foregroundColor(.primary)
                .frame(width: 34, alignment: .trailing)
        }
    }

    private func formatLufs(_ lufs: Double) -> String {
        if lufs.isNaN || lufs.isInfinite || lufs < -60 {
            return "-∞"
        }
        return String(format: "%.1f", lufs)
    }
}

/// Level meter group showing GPUI-style peak meters with separate LUFS rows.
struct LevelMeterView: View {
    let title: String
    let channelPeaks: [Double]
    let peakHolds: [Double]
    let channelLabels: [String]
    let momentaryLufs: Double
    let shortTermLufs: Double

    private let scaleWidth: CGFloat = 24
    private let barWidth: CGFloat = 16
    private let barSpacing: CGFloat = 1

    var body: some View {
        VStack(spacing: 6) {
            Text(title)
                .font(.system(size: 11, weight: .semibold))
                .foregroundColor(.secondary)

            HStack(alignment: .top, spacing: barSpacing) {
                VStack(spacing: 4) {
                    LevelMeterScale(width: scaleWidth)
                        .frame(maxHeight: .infinity)
                    Text("0")
                        .font(.system(size: 9, weight: .medium))
                        .opacity(0)
                }
                .frame(width: scaleWidth)

                ForEach(Array(channelPeaks.indices), id: \.self) { index in
                    let label = index < channelLabels.count ? channelLabels[index] : "\(index + 1)"
                    VStack(spacing: 4) {
                        LevelMeterBar(
                            level: channelPeaks[index],
                            peakHoldLevel: peakHoldValue(for: index),
                            width: barWidth
                        )
                        .frame(maxHeight: .infinity)

                        Text(label)
                            .font(.system(size: 9, weight: .medium))
                            .foregroundColor(.secondary)
                            .frame(width: barWidth)
                            .lineLimit(1)
                            .minimumScaleFactor(0.6)
                    }
                }
            }
            .frame(maxHeight: .infinity)
            .padding(.horizontal, 4)

            VStack(spacing: 2) {
                LufsMiniMeterRow(label: "M", value: momentaryLufs)
                LufsMiniMeterRow(label: "S", value: shortTermLufs)
            }
            .padding(.horizontal, 5)
            .padding(.bottom, 6)
        }
        .frame(maxHeight: .infinity)
        .padding(.top, 6)
        .background(Color(nsColor: .controlBackgroundColor).opacity(0.72))
        .cornerRadius(4)
    }

    private func peakHoldValue(for index: Int) -> Double? {
        index < peakHolds.count ? peakHolds[index] : nil
    }
}

// MARK: - Configuration View (SwiftUI)

struct ConfigurationView: View {
    let client: AudioEngineClient
    let onClose: () -> Void

    @State private var devices: [AudioEngineClient.AudioDevice] = []
    @State private var selectedDevice: String = ""
    @State private var volume: Float = 1.0
    // Audio Source Configuration
    @State private var availableSources: [AudioSource] = []
    @State private var selectedSource: AudioSource = .halDriver
    @State private var sourceDetectionStatus: [AudioSource: Bool] = [:]

    // HAL Configuration
    @State private var halInputChannels: Int = 2
    @State private var halOutputChannels: Int = 2

    // Error handling
    @State private var showingError = false
    @State private var errorMessage = ""

    // Level metering
    @State private var inputPeaks: [Double] = [0.0, 0.0]
    @State private var outputPeaks: [Double] = [0.0, 0.0]
    @State private var inputPeakHolds: [Double] = [0.0, 0.0]
    @State private var outputPeakHolds: [Double] = [0.0, 0.0]
    @State private var momentaryLufs: Double = -60.0
    @State private var shortTermLufs: Double = -60.0
    @State private var meteringTimer: Timer? = nil
    @State private var meteringRequestInFlight = false
    @State private var loadingDevices = false

    // Encryption state
    @State private var encryptionEnabled: Bool = true
    @State private var encryptionFingerprint: String = ""
    @State private var encryptionError: String? = nil
    @State private var pluginRackRefreshToken = 0

    // HAL Configuration state
    @State private var halConfig: AudioEngineClient.HalConfigData = AudioEngineClient.HalConfigData()
    @State private var selectedSampleRate: UInt32 = 48000
    @State private var selectedBufferFrames: UInt32 = 512
    @State private var halConfigError: String? = nil

    let channelOptions = Array(1...16)
    let sampleRateOptions: [UInt32] = [44100, 48000, 96000]
    let bufferFramesOptions: [UInt32] = [128, 256, 512, 1024, 2048]

    var body: some View {
        HStack(spacing: 0) {
            // Left level meter (input monitor)
            LevelMeterView(
                title: "Monitor In",
                channelPeaks: inputPeaks,
                peakHolds: inputPeakHolds,
                channelLabels: channelLabels(for: inputPeaks.count),
                momentaryLufs: momentaryLufs,
                shortTermLufs: shortTermLufs
            )
            .frame(width: meterWidth(for: inputPeaks.count))
            .padding(.leading, 8)

            // Main content with scroll
            VStack(spacing: 0) {
                // Scrollable configuration content
                ScrollView(.vertical, showsIndicators: true) {
                    VStack(spacing: 20) {

            // Audio Source Section
            GroupBox(label: Label("Audio Source", systemImage: "speaker.wave.3")) {
                VStack(alignment: .leading, spacing: 12) {
                    Text("Select how the daemon captures system audio")
                        .font(.caption)
                        .foregroundColor(.secondary)

                    ForEach(AudioSource.allCases) { source in
                        HStack {
                            Button(action: {
                                selectedSource = source
                                applyHALConfiguration()
                            }) {
                                HStack {
                                    Image(systemName: selectedSource == source ? "largecircle.fill.circle" : "circle")
                                        .foregroundColor(selectedSource == source ? .accentColor : .secondary)
                                    VStack(alignment: .leading) {
                                        Text(source.rawValue)
                                            .fontWeight(selectedSource == source ? .semibold : .regular)
                                        Text(source.description)
                                            .font(.caption)
                                            .foregroundColor(.secondary)
                                    }
                                }
                            }
                            .buttonStyle(.plain)

                            Spacer()

                            // Status indicator
                            if let isDetected = sourceDetectionStatus[source] {
                                if isDetected {
                                    HStack(spacing: 4) {
                                        Image(systemName: "checkmark.circle.fill")
                                            .foregroundColor(.green)
                                        Text("Detected")
                                            .font(.caption)
                                            .foregroundColor(.green)
                                    }
                                } else {
                                    HStack(spacing: 4) {
                                        Image(systemName: "exclamationmark.triangle.fill")
                                            .foregroundColor(.orange)
                                        Text("Not found")
                                            .font(.caption)
                                            .foregroundColor(.orange)
                                    }
                                }
                            }
                        }
                        .padding(.vertical, 4)
                    }

                    // Setup instructions for selected source
                    if selectedSource == .blackhole {
                        Divider()
                        VStack(alignment: .leading, spacing: 8) {
                            Label("Setup Instructions", systemImage: "info.circle")
                                .font(.headline)
                            Text(selectedSource.setupInstructions)
                                .font(.caption)
                                .foregroundColor(.secondary)
                                .padding(.leading, 20)
                        }
                        .padding(.top, 8)
                    }
                }
                .padding()
            }

            // HAL Input Section
            GroupBox(label: Label("HAL Input (from macOS apps)", systemImage: "waveform.path")) {
                VStack(alignment: .leading, spacing: 10) {
                    Text("The HAL driver captures audio from macOS applications")
                        .font(.caption)
                        .foregroundColor(.secondary)

                    HStack {
                        Text("Input Channels:")
                            .font(.headline)

                        Picker("", selection: $halInputChannels) {
                            ForEach(channelOptions, id: \.self) { count in
                                Text("\(count) channel\(count == 1 ? "" : "s")").tag(count)
                            }
                        }
                        .pickerStyle(.menu)
                        .frame(width: 150)
                        .onChange(of: halInputChannels) { _, _ in
                            applyHALConfiguration()
                        }

                        Spacer()

                        HStack(spacing: 4) {
                            Image(systemName: "info.circle")
                                .foregroundColor(.blue)
                            Text("Usually 2 for stereo")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                    }
                }
                .padding()
            }

            // Audio Output Section
            GroupBox(label: Label("Audio Output (to speakers)", systemImage: "hifispeaker")) {
                VStack(alignment: .leading, spacing: 10) {
                    Text("Output Device:")
                        .font(.headline)

                    Picker("Device", selection: $selectedDevice) {
                        if physicalOutputDevices.isEmpty {
                            Text("No hardware output devices").tag("")
                        }

                        ForEach(physicalOutputDevices, id: \.name) { device in
                            HStack {
                                Text(device.name)
                                if device.is_default {
                                    Text("(default)")
                                        .foregroundColor(.secondary)
                                        .font(.caption)
                                }
                                if let channels = device.channels, let sampleRate = device.sample_rate {
                                    Text("- \(channels)ch @ \(sampleRate/1000)kHz")
                                        .foregroundColor(.secondary)
                                        .font(.caption)
                                }
                            }
                            .tag(device.name)
                        }
                    }
                    .pickerStyle(.menu)
                    .onChange(of: selectedDevice) { _, newDevice in
                        guard !newDevice.isEmpty else { return }
                        guard !isVirtualDevice(newDevice) else {
                            errorMessage = "Virtual audio devices cannot be used as Systemwide speaker output. Select hardware speakers/headphones here, and select SotF Virtual Audio in macOS Sound Output."
                            showingError = true
                            loadDevices()
                            return
                        }
                        if !client.setDevice(newDevice) {
                            errorMessage = "Failed to set output device: \(newDevice)"
                            showingError = true
                            loadDevices()
                        }
                    }
                    .onAppear {
                        loadDevices()
                    }

                    Divider()

                    HStack {
                        Text("Output Channels:")
                            .font(.headline)

                        Picker("", selection: $halOutputChannels) {
                            ForEach(channelOptions, id: \.self) { count in
                                Text("\(count) channel\(count == 1 ? "" : "s")").tag(count)
                            }
                        }
                        .pickerStyle(.menu)
                        .frame(width: 150)
                        .onChange(of: halOutputChannels) { _, _ in
                            applyHALConfiguration()
                        }

                        Spacer()

                        HStack(spacing: 4) {
                            Image(systemName: "info.circle")
                                .foregroundColor(.blue)
                            Text("2=stereo, 5=5.0 surround, 6=5.1")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                    }

                    Divider()

                    HStack {
                        Text("Volume:")
                        Slider(value: $volume, in: 0...1)
                            .onChange(of: volume) { _, newVolume in
                                _ = client.setVolume(newVolume)
                            }
                        Text("\(Int(volume * 100))%")
                            .frame(width: 50)
                    }
                }
                .padding()
            }

            // Plugin Configuration Section
            GroupBox(label: Label("Audio Processing Plugins", systemImage: "slider.horizontal.3")) {
                VStack(alignment: .leading, spacing: 10) {
                    HStack {
                        Button("Load Configuration...") {
                            loadPluginConfig()
                        }

                        Button("Save Configuration...") {
                            savePluginConfig()
                        }

                        Spacer()
                    }

                    Divider()

                    PluginRackView(
                        client: client,
                        outputChannels: halOutputChannels,
                        refreshTrigger: pluginRackRefreshToken
                    )
                }
                .padding()
            }

            // Security Section
            GroupBox(label: Label("Security", systemImage: "lock.shield")) {
                VStack(alignment: .leading, spacing: 12) {
                    HStack {
                        Toggle("Encrypt audio data", isOn: $encryptionEnabled)
                            .onChange(of: encryptionEnabled) { _, newValue in
                                setEncryption(enabled: newValue)
                            }

                        Spacer()

                        // Status indicator
                        HStack(spacing: 4) {
                            Image(systemName: encryptionEnabled ? "lock.fill" : "lock.open")
                                .foregroundColor(encryptionEnabled ? .green : .secondary)
                            if !encryptionFingerprint.isEmpty {
                                Text(encryptionFingerprint.prefix(8) + "...")
                                    .font(.system(size: 10, design: .monospaced))
                                    .foregroundColor(.secondary)
                            }
                        }
                    }

                    Text("When enabled, audio data is encrypted in memory to prevent other users or processes from accessing it. Recommended for multi-user systems.")
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .padding(.leading, 20)

                    // Error state (conditional)
                    if let error = encryptionError {
                        HStack(spacing: 4) {
                            Image(systemName: "exclamationmark.triangle.fill")
                                .foregroundColor(.orange)
                            Text(error)
                                .font(.caption)
                                .foregroundColor(.orange)
                        }
                    }

                    Divider()

                    HStack {
                        Button("Rotate Key") {
                            rotateEncryptionKey()
                        }
                        .help("Generate a new encryption key")

                        Spacer()

                        Button("Refresh Status") {
                            refreshEncryptionStatus()
                        }
                    }
                }
                .padding()
            }
            .onAppear {
                refreshEncryptionStatus()
            }

            // HAL Driver Configuration Section
            GroupBox(label: Label("HAL Driver Configuration", systemImage: "cpu")) {
                VStack(alignment: .leading, spacing: 12) {
                    // Status row
                    HStack {
                        // Driver status
                        HStack(spacing: 4) {
                            Image(systemName: halConfig.driverReady ? "checkmark.circle.fill" : "xmark.circle.fill")
                                .foregroundColor(halConfig.driverReady ? .green : .red)
                            Text(halConfig.driverReady ? "Driver Ready" : "Driver Not Ready")
                                .font(.caption)
                        }

                        Spacer()

                        // Active status
                        HStack(spacing: 4) {
                            Image(systemName: halConfig.active ? "waveform" : "waveform.slash")
                                .foregroundColor(halConfig.active ? .green : .secondary)
                            Text(halConfig.active ? "Audio Active" : "No Audio")
                                .font(.caption)
                                .foregroundColor(halConfig.active ? .primary : .secondary)
                        }

                        Spacer()

                        Button(action: refreshHalConfig) {
                            Image(systemName: "arrow.clockwise")
                        }
                        .help("Refresh HAL status")
                    }

                    Divider()

                    // Sample Rate
                    HStack {
                        Text("Sample Rate:")
                            .frame(width: 100, alignment: .leading)

                        Picker("", selection: $selectedSampleRate) {
                            ForEach(sampleRateOptions, id: \.self) { rate in
                                Text("\(rate) Hz").tag(rate)
                            }
                        }
                        .pickerStyle(.segmented)
                        .frame(width: 250)
                        .onChange(of: selectedSampleRate) { _, newRate in
                            setSampleRate(newRate)
                        }

                        Spacer()

                        // Show actual rate if different from requested
                        if halConfig.actualSampleRate != selectedSampleRate && halConfig.actualSampleRate != 0 {
                            HStack(spacing: 4) {
                                Image(systemName: "arrow.right")
                                    .foregroundColor(.orange)
                                Text("Actual: \(halConfig.actualSampleRate) Hz")
                                    .font(.caption)
                                    .foregroundColor(.orange)
                            }
                        }
                    }

                    // Buffer Frames
                    HStack {
                        Text("Buffer Size:")
                            .frame(width: 100, alignment: .leading)

                        Picker("", selection: $selectedBufferFrames) {
                            ForEach(bufferFramesOptions, id: \.self) { frames in
                                Text("\(frames) frames").tag(frames)
                            }
                        }
                        .pickerStyle(.menu)
                        .frame(width: 150)
                        .onChange(of: selectedBufferFrames) { _, newFrames in
                            setBufferFrames(newFrames)
                        }

                        // Calculate latency
                        let latencyMs = Double(selectedBufferFrames) / Double(selectedSampleRate) * 1000.0
                        Text(String(format: "≈ %.1f ms latency", latencyMs))
                            .font(.caption)
                            .foregroundColor(.secondary)

                        Spacer()

                        // Show actual buffer if different
                        if halConfig.actualBufferFrames != selectedBufferFrames && halConfig.actualBufferFrames != 0 {
                            HStack(spacing: 4) {
                                Image(systemName: "arrow.right")
                                    .foregroundColor(.orange)
                                Text("Actual: \(halConfig.actualBufferFrames)")
                                    .font(.caption)
                                    .foregroundColor(.orange)
                            }
                        }
                    }

                    // Config negotiation status
                    if halConfig.configStatus != 0 {
                        HStack(spacing: 4) {
                            let (statusIcon, statusText, statusColor) = configStatusDisplay(halConfig.configStatus)
                            Image(systemName: statusIcon)
                                .foregroundColor(statusColor)
                            Text(statusText)
                                .font(.caption)
                                .foregroundColor(statusColor)

                            if halConfig.configSource == 1 {
                                Text("(HAL initiated)")
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                            } else if halConfig.configSource == 2 {
                                Text("(Daemon initiated)")
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                            }
                        }
                    }

                    // Error display
                    if let error = halConfigError {
                        HStack(spacing: 4) {
                            Image(systemName: "exclamationmark.triangle.fill")
                                .foregroundColor(.orange)
                            Text(error)
                                .font(.caption)
                                .foregroundColor(.orange)
                        }
                    }

                    // Info text
                    Text("Sample rate and buffer size affect audio quality and latency. Lower buffer sizes reduce latency but may cause audio glitches on slower systems.")
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .padding(.top, 4)
                }
                .padding()
            }
            .onAppear {
                refreshHalConfig()
            }
                    }  // End of scrollable VStack
                    .padding(.horizontal)
                    .padding(.top)
                    .padding(.bottom)
                }  // End of ScrollView

                // Status bar (fixed at bottom, not scrollable)
                Divider()
                HStack {
                    Image(systemName: "circle.fill")
                        .foregroundColor(.green)
                    Text("Connected to audio engine | Source: \(selectedSource.rawValue) | \(halInputChannels)ch in → \(halOutputChannels)ch out")
                        .foregroundColor(.secondary)
                }
                .padding()
            }  // End of main VStack

            // Right level meter (after plugin chain)
            LevelMeterView(
                title: "Post Chain",
                channelPeaks: outputPeaks,
                peakHolds: outputPeakHolds,
                channelLabels: channelLabels(for: outputPeaks.count),
                momentaryLufs: momentaryLufs,
                shortTermLufs: shortTermLufs
            )
            .frame(width: meterWidth(for: outputPeaks.count))
            .padding(.trailing, 8)
        }  // End of HStack
        .frame(minWidth: 820, minHeight: 600)
        .onAppear {
            loadDevices()
            startMeteringTimer()
        }
        .onDisappear {
            stopMeteringTimer()
        }
        .alert("Configuration Error", isPresented: $showingError) {
            Button("OK", role: .cancel) { }
        } message: {
            Text(errorMessage)
        }
    }

    private func meterWidth(for channelCount: Int) -> CGFloat {
        let count = max(channelCount, 1)
        return max(84, 34 + CGFloat(count * 17))
    }

    private func channelLabels(for channelCount: Int) -> [String] {
        switch max(channelCount, 1) {
        case 1:
            return ["M"]
        case 2:
            return ["L", "R"]
        case 4:
            return ["L", "R", "Ls", "Rs"]
        case 5:
            return ["L", "R", "C", "Ls", "Rs"]
        case 6:
            return ["L", "R", "C", "LFE", "Ls", "Rs"]
        case 8:
            return ["L", "R", "C", "LFE", "Ls", "Rs", "Lrs", "Rrs"]
        default:
            return (1...max(channelCount, 1)).map { "\($0)" }
        }
    }

    private func startMeteringTimer() {
        meteringTimer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { _ in
            updateMetering()
        }
    }

    private func stopMeteringTimer() {
        meteringTimer?.invalidate()
        meteringTimer = nil
    }

    private func updateMetering() {
        guard !meteringRequestInFlight else {
            let nextInputPeaks = decayedPeaks(inputPeaks)
            let nextOutputPeaks = decayedPeaks(outputPeaks)
            inputPeakHolds = updatedPeakHolds(previous: inputPeakHolds, current: nextInputPeaks)
            outputPeakHolds = updatedPeakHolds(previous: outputPeakHolds, current: nextOutputPeaks)
            inputPeaks = nextInputPeaks
            outputPeaks = nextOutputPeaks
            return
        }

        meteringRequestInFlight = true
        DispatchQueue.global(qos: .userInteractive).async {
            let metering = AudioEngineClient().getMetering()

            DispatchQueue.main.async {
                meteringRequestInFlight = false
                applyMetering(metering)
            }
        }
    }

    private func applyMetering(_ metering: AudioEngineClient.MeteringData?) {
        var nextInputPeaks = inputPeaks
        var nextOutputPeaks = outputPeaks

        if let metering = metering {
            // Input peaks from pre-processing monitor
            if let input = metering.input {
                if !input.channelPeaks.isEmpty {
                    nextInputPeaks = sanitizedPeaks(input.channelPeaks)
                } else {
                    nextInputPeaks = sanitizedPeaks(Array(repeating: input.peak, count: max(inputPeaks.count, 1)))
                }
            } else {
                nextInputPeaks = decayedPeaks(inputPeaks)
            }

            // Output peaks from post-processing monitor
            if let output = metering.output {
                if !output.channelPeaks.isEmpty {
                    nextOutputPeaks = sanitizedPeaks(output.channelPeaks)
                } else {
                    nextOutputPeaks = sanitizedPeaks(Array(repeating: output.peak, count: max(outputPeaks.count, 1)))
                }
                momentaryLufs = output.momentary
                shortTermLufs = output.shortTerm
            } else {
                nextOutputPeaks = decayedPeaks(outputPeaks)
            }
        } else {
            nextInputPeaks = decayedPeaks(inputPeaks)
            nextOutputPeaks = decayedPeaks(outputPeaks)
        }

        inputPeakHolds = updatedPeakHolds(previous: inputPeakHolds, current: nextInputPeaks)
        outputPeakHolds = updatedPeakHolds(previous: outputPeakHolds, current: nextOutputPeaks)
        inputPeaks = nextInputPeaks
        outputPeaks = nextOutputPeaks
    }

    private func sanitizedPeaks(_ peaks: [Double]) -> [Double] {
        peaks.map { peak in
            guard peak.isFinite, peak > 0 else {
                return 0.0
            }
            return min(peak, 2.0)
        }
    }

    private func decayedPeaks(_ peaks: [Double]) -> [Double] {
        peaks.map { peak in
            let next = peak * 0.85
            return next < 0.00001 ? 0.0 : next
        }
    }

    private func updatedPeakHolds(previous: [Double], current: [Double]) -> [Double] {
        current.enumerated().map { index, peak in
            let oldValue = index < previous.count ? previous[index] : 0.0
            if peak >= oldValue {
                return peak
            }
            let decayed = oldValue * 0.96
            return max(peak, decayed < 0.00001 ? 0.0 : decayed)
        }
    }

    /// Virtual device patterns that should not be used as speaker output.
    private let virtualDevicePatterns = [
        "SotF",
        "BlackHole",
        "Loopback",
        "Virtual",
        "Soundflower",
        "Background Music",
        "Audio Bridge",
        "ZoomAudio",
    ]

    /// Check if a device name matches a virtual device pattern
    private func isVirtualDevice(_ name: String) -> Bool {
        return virtualDevicePatterns.contains { pattern in
            name.range(of: pattern, options: [.caseInsensitive, .diacriticInsensitive]) != nil
        }
    }

    private var physicalOutputDevices: [AudioEngineClient.AudioDevice] {
        devices.filter { !isVirtualDevice($0.name) }
    }

    private func loadDevices() {
        guard !loadingDevices else { return }
        loadingDevices = true

        DispatchQueue.global(qos: .utility).async {
            var loadedDevices = AudioEngineClient().listDevices()
            if loadedDevices.isEmpty {
                loadedDevices = detectOutputDevicesViaCoreAudio()
            }

            DispatchQueue.main.async {
                loadingDevices = false
                devices = loadedDevices
                applyLoadedDevices()
            }
        }
    }

    private func applyLoadedDevices() {
        // Filter out virtual devices for output selection
        let physicalDevices = physicalOutputDevices

        // Prefer a physical device as output, avoiding virtual devices (HAL, BlackHole)
        if let physicalDefault = physicalDevices.first(where: { $0.is_default }) {
            // Use the physical default device
            selectedDevice = physicalDefault.name
        } else if let firstPhysical = physicalDevices.first {
            // Use the first physical device
            selectedDevice = firstPhysical.name
        } else {
            selectedDevice = ""
        }

        // Also detect available audio sources
        detectAvailableSources()
    }

    /// Detect which audio sources (HAL driver, BlackHole) are available on the system
    private func detectAvailableSources() {
        // Check each source type against available devices
        for source in AudioSource.allCases {
            let isDetected = devices.contains { device in
                device.name.contains(source.devicePattern)
            }
            sourceDetectionStatus[source] = isDetected
        }

        // Update available sources list
        availableSources = AudioSource.allCases.filter { source in
            sourceDetectionStatus[source] == true
        }

        // If current selection is not available, switch to first available
        if let firstAvailable = availableSources.first,
           sourceDetectionStatus[selectedSource] != true {
            selectedSource = firstAvailable
        }
    }

    /// Use Core Audio API to detect audio devices directly
    private func detectAudioDevicesViaCoreAudio() {
        var propertyAddress = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )

        var dataSize: UInt32 = 0
        var status = AudioObjectGetPropertyDataSize(
            AudioObjectID(kAudioObjectSystemObject),
            &propertyAddress,
            0,
            nil,
            &dataSize
        )

        guard status == noErr else {
            print("Failed to get audio devices data size: \(status)")
            return
        }

        let deviceCount = Int(dataSize) / MemoryLayout<AudioDeviceID>.size
        var deviceIDs = [AudioDeviceID](repeating: 0, count: deviceCount)

        status = AudioObjectGetPropertyData(
            AudioObjectID(kAudioObjectSystemObject),
            &propertyAddress,
            0,
            nil,
            &dataSize,
            &deviceIDs
        )

        guard status == noErr else {
            print("Failed to get audio devices: \(status)")
            return
        }

        // Check each device name
        for deviceID in deviceIDs {
            if let deviceName = getDeviceName(deviceID: deviceID) {
                for source in AudioSource.allCases {
                    if deviceName.contains(source.devicePattern) {
                        sourceDetectionStatus[source] = true
                    }
                }
            }
        }
    }

    /// Enumerate output devices directly through CoreAudio when the daemon is not reachable.
    private func detectOutputDevicesViaCoreAudio() -> [AudioEngineClient.AudioDevice] {
        var propertyAddress = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )

        var dataSize: UInt32 = 0
        var status = AudioObjectGetPropertyDataSize(
            AudioObjectID(kAudioObjectSystemObject),
            &propertyAddress,
            0,
            nil,
            &dataSize
        )

        guard status == noErr else {
            print("Failed to get CoreAudio device list size: \(status)")
            return []
        }

        let deviceCount = Int(dataSize) / MemoryLayout<AudioDeviceID>.size
        var deviceIDs = [AudioDeviceID](repeating: 0, count: deviceCount)

        status = AudioObjectGetPropertyData(
            AudioObjectID(kAudioObjectSystemObject),
            &propertyAddress,
            0,
            nil,
            &dataSize,
            &deviceIDs
        )

        guard status == noErr else {
            print("Failed to get CoreAudio device list: \(status)")
            return []
        }

        let defaultOutputDevice = getDefaultOutputDeviceID()
        var outputDevices: [AudioEngineClient.AudioDevice] = []
        var seenNames = Set<String>()

        for deviceID in deviceIDs {
            let channels = getOutputChannelCount(deviceID: deviceID)
            guard channels > 0,
                  let name = getDeviceName(deviceID: deviceID),
                  !seenNames.contains(name) else {
                continue
            }

            seenNames.insert(name)
            outputDevices.append(AudioEngineClient.AudioDevice(
                name: name,
                is_default: deviceID == defaultOutputDevice,
                channels: channels,
                sample_rate: getNominalSampleRate(deviceID: deviceID)
            ))
        }

        return outputDevices
    }

    private func getDefaultOutputDeviceID() -> AudioDeviceID {
        var propertyAddress = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )

        var deviceID = AudioDeviceID(0)
        var dataSize = UInt32(MemoryLayout<AudioDeviceID>.size)
        let status = AudioObjectGetPropertyData(
            AudioObjectID(kAudioObjectSystemObject),
            &propertyAddress,
            0,
            nil,
            &dataSize,
            &deviceID
        )

        return status == noErr ? deviceID : AudioDeviceID(0)
    }

    private func getOutputChannelCount(deviceID: AudioDeviceID) -> Int {
        var propertyAddress = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyStreamConfiguration,
            mScope: kAudioDevicePropertyScopeOutput,
            mElement: kAudioObjectPropertyElementMain
        )

        var dataSize: UInt32 = 0
        var status = AudioObjectGetPropertyDataSize(
            deviceID,
            &propertyAddress,
            0,
            nil,
            &dataSize
        )

        guard status == noErr, dataSize >= UInt32(MemoryLayout<AudioBufferList>.size) else {
            return 0
        }

        let bufferListPointer = UnsafeMutableRawPointer.allocate(
            byteCount: Int(dataSize),
            alignment: MemoryLayout<AudioBufferList>.alignment
        )
        defer { bufferListPointer.deallocate() }

        status = AudioObjectGetPropertyData(
            deviceID,
            &propertyAddress,
            0,
            nil,
            &dataSize,
            bufferListPointer
        )

        guard status == noErr else {
            return 0
        }

        let audioBufferList = bufferListPointer.bindMemory(to: AudioBufferList.self, capacity: 1)
        return UnsafeMutableAudioBufferListPointer(audioBufferList)
            .reduce(0) { $0 + Int($1.mNumberChannels) }
    }

    private func getNominalSampleRate(deviceID: AudioDeviceID) -> Int? {
        var propertyAddress = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyNominalSampleRate,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )

        var sampleRate = Float64(0)
        var dataSize = UInt32(MemoryLayout<Float64>.size)
        let status = AudioObjectGetPropertyData(
            deviceID,
            &propertyAddress,
            0,
            nil,
            &dataSize,
            &sampleRate
        )

        return status == noErr && sampleRate > 0 ? Int(sampleRate.rounded()) : nil
    }

    /// Get the name of an audio device by its ID
    private func getDeviceName(deviceID: AudioDeviceID) -> String? {
        var propertyAddress = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyDeviceNameCFString,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )

        var name: Unmanaged<CFString>?
        var dataSize = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)

        let status = withUnsafeMutablePointer(to: &name) { namePtr in
            AudioObjectGetPropertyData(
                deviceID,
                &propertyAddress,
                0,
                nil,
                &dataSize,
                namePtr
            )
        }

        if status == noErr, let cfName = name?.takeRetainedValue() {
            return cfName as String
        }
        return nil
    }

    private func applyHALConfiguration() {
        // Validate channel configuration
        guard halOutputChannels >= 1 && halOutputChannels <= 16 else {
            errorMessage = "Invalid output channel count: \(halOutputChannels). Must be between 1 and 16."
            showingError = true
            return
        }

        // Preserve the user's processing chain while changing driver output channels.
        // The daemon auto-injects the input/output loudness monitors.
        let plugins = client.getPlugins() ?? []

        let command: [String: Any] = [
            "command": "load_plugins",
            "plugins": plugins,
            "output_channels": halOutputChannels
        ]

        guard let response = client.sendCommand(command) else {
            errorMessage = "Failed to communicate with daemon. Please ensure the daemon is running."
            showingError = true
            return
        }

        if response.success {
            print("✅ HAL configuration applied: \(halOutputChannels)ch out")
        } else {
            errorMessage = response.error ?? "Unknown error occurred while applying HAL configuration."
            showingError = true
        }
    }

    private func loadPluginConfig() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.json]
        panel.allowsMultipleSelection = false
        panel.message = "Select plugin configuration file"

        if panel.runModal() == .OK, let url = panel.url {
            do {
                let data = try Data(contentsOf: url)
                let json = try JSONSerialization.jsonObject(with: data)
                let userPlugins = try normalizedPluginConfigs(from: json)

                // Send plugins to daemon (daemon auto-injects metering)
                let command: [String: Any] = [
                    "command": "load_plugins",
                    "plugins": userPlugins,
                    "output_channels": halOutputChannels
                ]

                let response = client.sendCommand(command)
                if let resp = response, resp.success {
                    print("✅ Plugin configuration loaded from: \(url.path)")
                    print("   User plugins: \(userPlugins.count)")
                    pluginRackRefreshToken += 1
                } else {
                    errorMessage = response?.error ?? "Failed to apply plugin configuration"
                    showingError = true
                }
            } catch {
                errorMessage = "Failed to read configuration: \(error.localizedDescription)"
                showingError = true
            }
        }
    }

    private func savePluginConfig() {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.json]
        panel.nameFieldStringValue = "sotf_systemwide_plugins.json"
        panel.message = "Save plugin configuration"

        if panel.runModal() == .OK, let url = panel.url {
            // Query daemon for the current active plugin list
            if let currentPlugins = client.getPlugins() {
                do {
                    let data = try JSONSerialization.data(
                        withJSONObject: appGpuiPreset(from: currentPlugins),
                        options: .prettyPrinted
                    )
                    try data.write(to: url)
                    print("✅ Plugin configuration saved to: \(url.path)")
                } catch {
                    errorMessage = "Failed to save configuration: \(error.localizedDescription)"
                    showingError = true
                }
            } else {
                errorMessage = "Failed to retrieve current plugin list from daemon"
                showingError = true
            }
        }
    }

    private func normalizedPluginConfigs(from json: Any) throws -> [[String: Any]] {
        if let simplePlugins = json as? [[String: Any]] {
            return simplePlugins.compactMap(normalizedPluginConfigEntry)
        }

        guard let configDict = json as? [String: Any] else {
            throw pluginConfigError("Invalid configuration format: expected an array or object")
        }

        if let plugins = configDict["plugins"] as? [[String: Any]] {
            return plugins.compactMap(normalizedPluginConfigEntry)
        }

        var allPlugins: [[String: Any]] = []
        if let globalPlugins = configDict["global_plugins"] as? [[String: Any]] {
            allPlugins.append(contentsOf: globalPlugins)
        }

        if let channels = configDict["channels"] as? [String: Any] {
            for channelName in channels.keys.sorted() {
                if let channelData = channels[channelName] as? [String: Any],
                   let channelPlugins = channelData["plugins"] as? [[String: Any]] {
                    allPlugins.append(contentsOf: channelPlugins)
                }
            }
            print("Found \(channels.count) channel(s) with \(allPlugins.count) total plugin entries")
        }

        if allPlugins.isEmpty {
            throw pluginConfigError("No plugins found in configuration")
        }

        return allPlugins.compactMap(normalizedPluginConfigEntry)
    }

    private func normalizedPluginConfigEntry(_ entry: [String: Any]) -> [String: Any]? {
        if let type = (entry["plugin_type"] as? String) ?? (entry["type"] as? String) {
            if isSystemPluginType(type) {
                return nil
            }
            return [
                "plugin_type": type,
                "parameters": entry["parameters"] as? [String: Any] ?? [:],
            ]
        }

        guard let settings = entry["settings"] else {
            return nil
        }

        let enabled = entry["enabled"] as? Bool ?? true
        let permanent = entry["permanent"] as? Bool ?? false
        return pluginConfigFromAppGpuiSettings(settings, enabled: enabled, permanent: permanent)
    }

    private func pluginConfigFromAppGpuiSettings(_ settings: Any, enabled: Bool, permanent: Bool) -> [String: Any]? {
        guard enabled, !permanent else {
            return nil
        }

        let variant: String
        let parameters: [String: Any]

        if let variantName = settings as? String {
            variant = variantName
            parameters = [:]
        } else if let settingsDict = settings as? [String: Any],
                  let first = settingsDict.first {
            variant = first.key
            parameters = first.value as? [String: Any] ?? [:]
        } else {
            return nil
        }

        guard let type = appGpuiSettingsVariantToEngineType[variant],
              !isSystemPluginType(type) else {
            return nil
        }

        return [
            "plugin_type": type,
            "parameters": parameters,
        ]
    }

    private func appGpuiPreset(from plugins: [[String: Any]]) -> [String: Any] {
        let available = client.getAvailablePlugins() ?? []
        let defaultsByType = Dictionary(uniqueKeysWithValues: available.map { ($0.type_, $0.defaultParameters) })

        let records = plugins.enumerated().compactMap { index, plugin in
            appGpuiPluginRecord(from: plugin, id: index, defaultsByType: defaultsByType)
        }

        return [
            "version": 2,
            "plugins": records,
        ]
    }

    private func appGpuiPluginRecord(
        from plugin: [String: Any],
        id: Int,
        defaultsByType: [String: [String: Any]]
    ) -> [String: Any]? {
        guard let type = plugin["plugin_type"] as? String,
              !isSystemPluginType(type),
              let variant = engineTypeToAppGpuiSettingsVariant[type] else {
            return nil
        }

        var parameters = defaultsByType[type] ?? [:]
        if let currentParameters = plugin["parameters"] as? [String: Any] {
            for (key, value) in currentParameters {
                parameters[key] = value
            }
        }

        var record: [String: Any] = [
            "id": id,
            "enabled": true,
            "settings": [variant: parameters],
            "permanent": false,
            "plugin_type": type,
            "parameters": parameters,
        ]

        if let name = plugin["name"] as? String, !name.isEmpty {
            record["name"] = name
        }

        return record
    }

    private func isSystemPluginType(_ type: String) -> Bool {
        return type == "hal_input"
            || type == "hal_output"
            || type == "loudness_monitor"
            || type == "spectrum_analyzer"
    }

    private func pluginConfigError(_ message: String) -> NSError {
        NSError(
            domain: "SotFSystemwidePluginConfig",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: message]
        )
    }

    // MARK: - Encryption Methods

    private func setEncryption(enabled: Bool) {
        encryptionError = nil

        if client.setEncryption(enabled: enabled) {
            print("✅ Encryption \(enabled ? "enabled" : "disabled")")
            refreshEncryptionStatus()
        } else {
            encryptionError = "Failed to \(enabled ? "enable" : "disable") encryption"
            // Revert the toggle state
            encryptionEnabled = !enabled
        }
    }

    private func rotateEncryptionKey() {
        encryptionError = nil

        if client.rotateEncryptionKey() {
            print("✅ Encryption key rotated")
            refreshEncryptionStatus()
        } else {
            encryptionError = "Failed to rotate encryption key"
        }
    }

    private func refreshEncryptionStatus() {
        DispatchQueue.global(qos: .utility).async {
            let status = AudioEngineClient().getEncryptionStatus()

            DispatchQueue.main.async {
                if let status = status {
                    encryptionEnabled = status.enabled
                    encryptionFingerprint = status.fingerprint
                    encryptionError = nil
                } else {
                    // Daemon might not be running
                    encryptionFingerprint = ""
                }
            }
        }
    }

    // MARK: - HAL Configuration Methods

    private func refreshHalConfig() {
        halConfigError = nil

        DispatchQueue.global(qos: .utility).async {
            let config = AudioEngineClient().getHalConfig()

            DispatchQueue.main.async {
                if let config = config {
                    halConfig = config
                    // Update UI to match actual values
                    if config.actualSampleRate != 0 {
                        selectedSampleRate = config.actualSampleRate
                    }
                    if config.actualBufferFrames != 0 {
                        selectedBufferFrames = config.actualBufferFrames
                    }
                } else {
                    halConfigError = "Failed to get HAL config (daemon may not be running)"
                }
            }
        }
    }

    private func setSampleRate(_ rate: UInt32) {
        halConfigError = nil

        if client.setSampleRate(rate) {
            print("Sample rate set to \(rate) Hz")
            // Refresh to get actual values after negotiation
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                refreshHalConfig()
            }
        } else {
            halConfigError = "Failed to set sample rate"
            // Revert to previous value
            selectedSampleRate = halConfig.actualSampleRate != 0 ? halConfig.actualSampleRate : 48000
        }
    }

    private func setBufferFrames(_ frames: UInt32) {
        halConfigError = nil

        if client.setBufferFrames(frames) {
            print("Buffer frames set to \(frames)")
            // Refresh to get actual values after negotiation
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                refreshHalConfig()
            }
        } else {
            halConfigError = "Failed to set buffer frames"
            // Revert to previous value
            selectedBufferFrames = halConfig.actualBufferFrames != 0 ? halConfig.actualBufferFrames : 512
        }
    }

    /// Get display info for config status code
    private func configStatusDisplay(_ status: UInt32) -> (icon: String, text: String, color: Color) {
        switch status {
        case 0:
            return ("clock", "Pending...", .orange)
        case 1:
            return ("checkmark.circle", "Accepted", .green)
        case 2:
            return ("arrow.triangle.2.circlepath", "Negotiated", .blue)
        case 3:
            return ("xmark.circle", "Error", .red)
        default:
            return ("questionmark.circle", "Unknown", .secondary)
        }
    }
}

// MARK: - App Delegate

class AppDelegate: NSObject, NSApplicationDelegate {
    var statusBarController: StatusBarController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Write debug file FIRST before anything else
        do {
            let debugPath = NSHomeDirectory() + "/sotf-configbar-debug.log"
            try "applicationDidFinishLaunching called at \(Date())\n".write(toFile: debugPath, atomically: true, encoding: .utf8)
        } catch {
            // Can't even write file
        }

        // Hide dock icon (menu bar only app)
        NSApp.setActivationPolicy(.accessory)

        // Create status bar controller (which starts the daemon automatically)
        statusBarController = StatusBarController()
        print("Created statusBarController: \(String(describing: statusBarController))")

        // Show startup notification
        NotificationManager.shared.showNotification(
            title: "SotF Started",
            body: "Audio engine control ready"
        )

        print("SotF Systemwide menu bar app started")
    }

    func applicationWillTerminate(_ notification: Notification) {
        statusBarController?.stopMonitoring()
        statusBarController?.stopDaemon()
        print("SotF Systemwide menu bar app terminated")
    }
}

// MARK: - Notification Manager

class NotificationManager: NSObject, UNUserNotificationCenterDelegate {
    static let shared = NotificationManager()
    private var notificationsAvailable = false

    private override init() {
        super.init()
        setupNotifications()
    }

    private func setupNotifications() {
        // UNUserNotificationCenter requires a proper app bundle
        // Check if we're running from a valid bundle before trying to use it
        guard Bundle.main.bundleIdentifier != nil else {
            print("Notifications not available (not running from app bundle)")
            return
        }

        // Try to access UNUserNotificationCenter, which may fail without a proper bundle
        do {
            let center = UNUserNotificationCenter.current()
            center.delegate = self
            notificationsAvailable = true

            center.requestAuthorization(options: [.alert, .sound, .badge]) { granted, error in
                if granted {
                    print("Notification permission granted")
                } else if let error = error {
                    print("Notification permission error: \(error)")
                }
            }
        }
    }

    func showNotification(title: String, body: String, sound: Bool = true) {
        guard notificationsAvailable else {
            print("Notification (disabled): \(title) - \(body)")
            return
        }

        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        if sound {
            content.sound = .default
        }

        let request = UNNotificationRequest(
            identifier: UUID().uuidString,
            content: content,
            trigger: nil
        )

        UNUserNotificationCenter.current().add(request) { error in
            if let error = error {
                print("Error showing notification: \(error)")
            }
        }
    }

    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        completionHandler([.banner, .sound])
    }
}

// MARK: - Main

@main
struct SotFToolbarApp {
    static func main() {
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        app.run()
    }
}
