//
// SotF Systemwide Menu Bar Application
//
// A macOS menu bar app that controls the SotF Systemwide audio engine with:
// - Menu bar status icon with health tint, streaming background, and recording indicator
// - Configuration window for audio interfaces and plugin chains
// - Energy optimization (stops engine after 3s of silence)
// - Integration with src-audio daemon via Unix socket

import SwiftUI
import Cocoa
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
    private static let mutationQueue = DispatchQueue(
        label: "org.spinorama.sotf.configbar.daemon-mutations",
        qos: .userInitiated
    )
    // Status and metering share one serialized client. The daemon protocol
    // permits multiple JSON lines per connection; keeping this connection
    // alive avoids spawning a daemon thread for every 100 ms meter tick.
    private static let pollingQueue = DispatchQueue(
        label: "org.spinorama.sotf.configbar.daemon-polling",
        qos: .utility
    )
    private static let pollingClient = AudioEngineClient()
    /// Get the secure socket path (per-user directory)
    static func getSecureSocketPath() -> String {
        let environment = ProcessInfo.processInfo.environment
        if let overridePath = environment["SOTF_DAEMON_SOCKET_PATH"], !overridePath.isEmpty {
            return overridePath
        }
        if let runtimeDir = environment["SOTF_SYSTEMWIDE_RUNTIME_DIR"], !runtimeDir.isEmpty {
            return (runtimeDir as NSString).appendingPathComponent("daemon.sock")
        }

        // On macOS, TMPDIR is per-user and already secured
        if let tmpdir = environment["TMPDIR"] {
            return (tmpdir as NSString).appendingPathComponent("sotf-daemon.sock")
        }
        // Fallback to UID-based path
        return "/tmp/sotf-\(getuid())/daemon.sock"
    }

    /// The daemon owns one per-user socket. Do not fall back to the removed
    /// world-writable legacy path.
    private var socketPath: String {
        Self.getSecureSocketPath()
    }

    private var socketFD: Int32 = -1
    private(set) var lastCommandSucceeded = false

    enum AudioState: String {
        case idle = "Idle"
        case playing = "Playing"
        case recording = "Recording"
        case paused = "Paused"
        case stopped = "Stopped"
        case error = "Error"

        var iconColor: NSColor {
            switch self {
            case .recording:
                return .white
            case .error:
                return .black
            default:
                return .white
            }
        }

        var isStreaming: Bool {
            self == .playing || self == .recording
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
        var metadata = stat()
        guard Darwin.lstat(socketPath, &metadata) == 0,
              (metadata.st_mode & S_IFMT) == S_IFSOCK else {
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

        let maxSocketPathBytes = MemoryLayout.size(ofValue: addr.sun_path)
        guard !socketPath.utf8.contains(0),
              socketPath.utf8.count < maxSocketPathBytes else {
            print("Daemon socket path is too long or contains an embedded NUL: \(socketPath)")
            closeConnection()
            return false
        }

        let copiedPathLength = withUnsafeMutableBytes(of: &addr.sun_path) { pathBuffer -> Int in
            guard let baseAddress = pathBuffer.baseAddress else { return -1 }
            return socketPath.withCString { pathCString in
                Int(strlcpy(
                    baseAddress.assumingMemoryBound(to: CChar.self),
                    pathCString,
                    pathBuffer.count
                ))
            }
        }
        guard copiedPathLength == socketPath.utf8.count else {
            print("Failed to copy daemon socket path without truncation")
            closeConnection()
            return false
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

    private func ensureConnected() -> Bool {
        socketFD >= 0 || connect()
    }

    private func closeConnection() {
        if socketFD >= 0 {
            close(socketFD)
            socketFD = -1
        }
    }

    deinit {
        closeConnection()
    }

    func sendCommand(_ command: [String: Any]) -> Response? {
        sendCommand(command, keepConnection: false)
    }

    private func sendCommand(
        _ command: [String: Any],
        keepConnection: Bool
    ) -> Response? {
        lastCommandSucceeded = false
        let connected = keepConnection ? ensureConnected() : connect()
        guard connected else {
            return nil
        }

        var persistentConnectionHealthy = false
        defer {
            // A persistent connection is retained only after a complete,
            // successfully decoded response. Any framing/IO failure forces a
            // reconnect for the next poll.
            if !keepConnection || !persistentConnectionHealthy {
                closeConnection()
            }
        }

        do {
            // Send command
            let jsonData = try JSONSerialization.data(withJSONObject: command)
            guard let jsonString = String(data: jsonData, encoding: .utf8) else {
                print("Failed to encode daemon command as UTF-8")
                return nil
            }
            let jsonLine = jsonString + "\n"
            let commandBytes = [UInt8](jsonLine.utf8)

            guard ConfigBarIPC.writeAll(
                fd: socketFD,
                data: Data(commandBytes)
            ) else {
                print("Failed to send command: \(String(cString: strerror(errno)))")
                return nil
            }

            // Read response with buffered line-based parsing
            // TCP streams may fragment data, so we need to read until we find a newline
            var responseData = Data()
            var buffer = [UInt8](repeating: 0, count: 4096)
            let bufferCount = buffer.count
            let maxResponseSize = 1024 * 1024 // Plugin metadata can exceed 64KB
            let responseTimeoutMicros: useconds_t = 1000000 // Keep UI-facing calls from hanging the app

            // Set socket to non-blocking for timeout handling
            let flags = fcntl(socketFD, F_GETFL, 0)
            _ = fcntl(socketFD, F_SETFL, flags | O_NONBLOCK)

            var totalWaitTime: useconds_t = 0
            let pollInterval: useconds_t = 5000 // 5ms poll interval

            var lineFramer = ConfigBarLineFramer(maxLineBytes: maxResponseSize)

            while totalWaitTime < responseTimeoutMicros {
                let bytesRead = buffer.withUnsafeMutableBufferPointer { bufferPtr in
                    Darwin.recv(socketFD, bufferPtr.baseAddress, bufferCount, 0)
                }

                if bytesRead > 0 {
                    do {
                        try lineFramer.append(Data(buffer[0..<bytesRead]))
                    } catch ConfigBarIPCError.lineTooLong {
                        print("Daemon response exceeded \(maxResponseSize) bytes without a complete JSON line")
                        return nil
                    }
                    if let line = lineFramer.nextLine() {
                        responseData = line
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

            // Restore blocking mode
            _ = fcntl(socketFD, F_SETFL, flags)

            // The daemon uses newline framing, but accepting a complete JSON
            // response on an orderly EOF keeps the client tolerant of simple
            // test doubles and older daemon builds.
            if responseData.isEmpty {
                responseData = lineFramer.bufferedData
            }

            guard !responseData.isEmpty else {
                print("Empty response from daemon (timeout or connection closed)")
                return nil
            }

            let response = try JSONDecoder().decode(Response.self, from: responseData)
            lastCommandSucceeded = true
            persistentConnectionHealthy = true
            return response
        } catch {
            print("Failed to send command: \(error)")
        }

        return nil
    }

    private func sendPersistentCommand(_ command: [String: Any]) -> Response? {
        sendCommand(command, keepConnection: true)
    }

    /// Execute a mutating command off the UI thread and deliver its response
    /// on the main queue. Each operation gets its own client so the existing
    /// short-lived socket framing remains isolated while mutations are
    /// serialized in order.
    func sendCommandAsync(
        _ command: [String: Any],
        completion: @escaping (Response?) -> Void
    ) {
        let operationClient = AudioEngineClient()
        ConfigBarAsyncOperation.perform(on: Self.mutationQueue, work: {
            operationClient.sendCommand(command)
        }, completion: { response in
            completion(response)
        })
    }

    struct Status {
        let state: AudioState
        let volume: Float
        let muted: Bool
        let selectedDevice: String?
        let sampleRate: Int?
        let inputChannels: Int?
        let outputChannels: Int?
        let channels: Int?
        let playbackCallbackCount: Int?
        let playbackBufferFillPercent: Int?
        let playbackStreamErrorCount: Int?
        let playbackFramesReceived: Int?
        let playbackFramesWritten: Int?
        let playbackFramesDropped: Int?
        let playbackEffectiveSampleRate: Int?

        static let fallback = Status(
            state: .idle,
            volume: 1.0,
            muted: false,
            selectedDevice: nil,
            sampleRate: nil,
            inputChannels: nil,
            outputChannels: nil,
            channels: nil,
            playbackCallbackCount: nil,
            playbackBufferFillPercent: nil,
            playbackStreamErrorCount: nil,
            playbackFramesReceived: nil,
            playbackFramesWritten: nil,
            playbackFramesDropped: nil,
            playbackEffectiveSampleRate: nil
        )
    }

    func getStatus(reuseConnection: Bool = false) -> Status {
        let command = ["command": "status"]

        let response = reuseConnection
            ? sendPersistentCommand(command)
            : sendCommand(command)
        guard let response,
              response.success,
              let data = response.data else {
            return .fallback
        }

        let stateStr = data["state"]?.value as? String ?? "Idle"
        let state = AudioState(rawValue: stateStr) ?? .idle
        let volume = (data["volume"]?.value as? Double).map { Float($0) } ?? 1.0
        let muted = data["muted"]?.value as? Bool ?? false
        let selectedDevice = data["selected_device"]?.value as? String
        let sampleRate = data["sample_rate"]?.value as? Int
        let inputChannels = data["input_channels"]?.value as? Int
        let outputChannels = data["output_channels"]?.value as? Int
        let channels = data["channels"]?.value as? Int
        let playbackCallbackCount = data["playback_callback_count"]?.value as? Int
        let playbackBufferFillPercent = data["playback_buffer_fill_percent"]?.value as? Int
        let playbackStreamErrorCount = data["playback_stream_error_count"]?.value as? Int
        let playbackFramesReceived = data["playback_frames_received"]?.value as? Int
        let playbackFramesWritten = data["playback_frames_written"]?.value as? Int
        let playbackFramesDropped = data["playback_frames_dropped"]?.value as? Int
        let playbackEffectiveSampleRate = data["playback_effective_sample_rate"]?.value as? Int

        return Status(
            state: state,
            volume: volume,
            muted: muted,
            selectedDevice: selectedDevice,
            sampleRate: sampleRate,
            inputChannels: inputChannels,
            outputChannels: outputChannels,
            channels: channels,
            playbackCallbackCount: playbackCallbackCount,
            playbackBufferFillPercent: playbackBufferFillPercent,
            playbackStreamErrorCount: playbackStreamErrorCount,
            playbackFramesReceived: playbackFramesReceived,
            playbackFramesWritten: playbackFramesWritten,
            playbackFramesDropped: playbackFramesDropped,
            playbackEffectiveSampleRate: playbackEffectiveSampleRate
        )
    }

    /// Probe the daemon without assuming ownership of its process. A live
    /// launchd/debug daemon is adopted by the toolbar instead of being
    /// terminated and replaced.
    func isDaemonReachable() -> Bool {
        ConfigBarIPC.probeDaemon(socketPath: socketPath)
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

    func getMetering(reuseConnection: Bool = false) -> MeteringData? {
        let command: [String: Any] = ["command": "get_metering"]

        let response = reuseConnection
            ? sendPersistentCommand(command)
            : sendCommand(command)
        guard let response,
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

    static func pollStatus(completion: @escaping (Status, Bool) -> Void) {
        pollingQueue.async {
            let status = pollingClient.getStatus(reuseConnection: true)
            let reachable = pollingClient.lastCommandSucceeded
            DispatchQueue.main.async {
                completion(status, reachable)
            }
        }
    }

    static func pollMetering(completion: @escaping (MeteringData?) -> Void) {
        pollingQueue.async {
            let metering = pollingClient.getMetering(reuseConnection: true)
            DispatchQueue.main.async {
                completion(metering)
            }
        }
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

    /// Get the daemon-owned rack or graph artifact without flattening topology.
    func getPluginPipeline() -> PluginPipelineModel? {
        let command: [String: Any] = ["command": "get_plugins"]

        guard let response = sendCommand(command),
              response.success,
              let data = response.data else {
            return nil
        }

        let topology = PluginPipelineTopology(
            rawValue: data["topology"]?.value as? String ?? "rack"
        ) ?? .rack
        let plugins = (data["plugins"]?.value as? [Any] ?? [])
            .compactMap { $0 as? [String: Any] }
        let generation = numberValue(data["generation"]?.value).map(Int.init)

        var graphModel: PluginGraphModel?
        if topology == .graph,
           let graph = data["graph"]?.value as? [String: Any],
           let rawNodes = graph["nodes"] as? [Any],
           let rawEdges = graph["edges"] as? [Any] {
            let nodes = rawNodes.compactMap { raw -> PluginGraphNodeModel? in
                guard let node = raw as? [String: Any],
                      let id = numberValue(node["id"]).map(Int.init),
                      let pluginType = node["plugin_type"] as? String,
                      let inputChannels = numberValue(node["input_channels"]).map(Int.init)
                else {
                    return nil
                }
                return PluginGraphNodeModel(
                    id: id,
                    pluginType: pluginType,
                    parameters: node["parameters"] as? [String: Any] ?? [:],
                    inputChannels: inputChannels,
                    bypassed: node["bypassed"] as? Bool ?? false
                )
            }
            let edges = rawEdges.compactMap { raw -> PluginGraphEdgeModel? in
                guard let edge = raw as? [String: Any],
                      let fromNode = numberValue(edge["from_node"]).map(Int.init),
                      let toNode = numberValue(edge["to_node"]).map(Int.init)
                else {
                    return nil
                }
                return PluginGraphEdgeModel(fromNode: fromNode, toNode: toNode)
            }
            graphModel = PluginGraphModel(nodes: nodes, edges: edges)
        }

        return PluginPipelineModel(
            topology: topology,
            plugins: plugins,
            graph: graphModel,
            generation: generation
        )
    }

    /// Compatibility helper for rack-only callers.
    func getPlugins() -> [[String: Any]]? {
        guard let pipeline = getPluginPipeline(), pipeline.topology == .rack else {
            return nil
        }
        return pipeline.plugins
    }

    func loadPluginGraph(_ graph: PluginGraphModel, baseGeneration: Int?) -> Bool {
        var command: [String: Any] = [
            "command": "load_plugin_artifact",
            "artifact": ["graph": graph.artifact],
        ]
        if let baseGeneration {
            command["base_generation"] = baseGeneration
        }
        return sendCommand(command)?.success ?? false
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

    private func uint64Value(_ raw: Any?) -> UInt64? {
        if let value = raw as? UInt64 {
            return value
        }
        if let value = raw as? Int, value >= 0 {
            return UInt64(value)
        }
        if let value = raw as? NSNumber {
            let double = value.doubleValue
            guard double.isFinite, double >= 0, double.rounded() == double,
                  double <= Double(UInt64.max) else {
                return nil
            }
            return UInt64(double)
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
        status.frameCount = uint64Value(data["frame_count"]?.value) ?? 0

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
    private var startupProbeInFlight = false
    private var restartRequested = false

    /// Callback when daemon status changes
    var onStatusChange: ((Bool) -> Void)?

    private var daemonLogURL: URL {
        let appSupport = FileManager.default.urls(
            for: .applicationSupportDirectory,
            in: .userDomainMask
        ).first ?? URL(fileURLWithPath: NSTemporaryDirectory())
        return appSupport
            .appendingPathComponent("org.spinorama.sotf")
            .appendingPathComponent("sotf-daemon.log")
    }

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

    /// Start the daemon if not already running
    func startDaemon() {
        guard !isShuttingDown else { return }

        // Check if already running (our managed process)
        if let process = daemonProcess, process.isRunning {
            print("DaemonManager: Daemon already running (PID: \(process.processIdentifier))")
            return
        }

        guard !startupProbeInFlight else { return }
        startupProbeInFlight = true

        // The probe is a bounded IPC operation. Keep it off the main thread
        // so startup and reconnect remain responsive while the daemon is
        // down or a stale socket is present.
        DispatchQueue.global(qos: .utility).async { [weak self] in
            let reachable = AudioEngineClient().isDaemonReachable()
            DispatchQueue.main.async {
                guard let self else { return }
                self.startupProbeInFlight = false
                guard !self.isShuttingDown else { return }

                // Adopt any live daemon, including one launched by launchd or
                // a developer. The toolbar must not kill processes it did not
                // start.
                if ConfigBarDaemonAdoption.shouldAdopt(
                    reachable: reachable,
                    managedProcessRunning: self.daemonProcess?.isRunning ?? false
                ) {
                    print("DaemonManager: Adopting existing live daemon")
                    self.onStatusChange?(true)
                    self.startWatchdog()
                    return
                }

                // A managed process may have become visible while the probe
                // was in flight. Do not launch a second daemon in that race.
                guard self.daemonProcess?.isRunning != true else { return }

                self.launchDaemon()
            }
        }
    }

    private func launchDaemon() {
        guard !isShuttingDown else { return }

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
        let appSupportDir = daemonLogURL.deletingLastPathComponent()
        try? FileManager.default.createDirectory(at: appSupportDir, withIntermediateDirectories: true)
        let logPath = daemonLogURL.path
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

                let shouldRestart = self?.restartRequested == true
                    || (!(self?.isShuttingDown ?? true) && proc.terminationStatus != 0)
                self?.restartRequested = false
                if shouldRestart && !(self?.isShuttingDown ?? true) {
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

    /// Restart the managed daemon after an outage without killing or
    /// disturbing a live daemon adopted from launchd or another owner.
    func restartDaemon() {
        isShuttingDown = false
        restartRequested = true
        stopWatchdog()

        guard let process = daemonProcess, process.isRunning else {
            restartRequested = false
            startDaemon()
            return
        }

        process.terminate()
        DispatchQueue.global().asyncAfter(deadline: .now() + 1.0) {
            if process.isRunning {
                print("DaemonManager: Restart did not exit after SIGTERM; sending SIGINT fallback...")
                process.interrupt()
            }
        }
    }

    /// Open the daemon log in the user's configured application.
    func openDaemonLog() {
        NSWorkspace.shared.open(daemonLogURL)
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
                    print("DaemonManager: Daemon did not exit after SIGTERM; sending SIGINT fallback...")
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
    private static let recordingDotLayerName = "SotFRecordingDotLayer"

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
            button.wantsLayer = true
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
                // The shared polling client reconnects lazily after startup.
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
        updateIcon()
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

    /// Load the menu bar icon from the bundled PNG assets so updates to
    /// crates/systemwide/crates/daemon/configbar/assets/* propagate without
    /// editing Swift. Combines the 1x and @2x reps into a single template
    /// NSImage; AppKit picks the correct rep for the screen and tints it
    /// for the menu bar appearance.
    private func makeMenuBarIcon() -> NSImage {
        let logicalSize = NSSize(width: 22, height: 22)
        let image = NSImage(size: logicalSize)

        let bundle = Bundle.main
        let names: [(String, String)] = [
            ("icon_22", "png"),
            ("icon_22@2x", "png"),
        ]

        var added = 0
        for (name, ext) in names {
            guard
                let path = bundle.path(forResource: name, ofType: ext),
                let data = try? Data(contentsOf: URL(fileURLWithPath: path)),
                let rep = NSBitmapImageRep(data: data)
            else {
                continue
            }
            // Logical (point) size stays 22x22 for both reps; the @2x rep
            // keeps its 44x44 pixel backing because pixelsWide/pixelsHigh
            // are inferred from the file.
            rep.size = logicalSize
            image.addRepresentation(rep)
            added += 1
        }

        if added == 0 {
            // Asset files missing — fall back to a system symbol so the
            // status item still has something visible.
            if let fallback = NSImage(
                systemSymbolName: "headphones",
                accessibilityDescription: "SotF"
            ) {
                fallback.isTemplate = true
                return fallback
            }
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

        AudioEngineClient.pollStatus { [weak self] status, reachable in
            guard let self = self else { return }
            self.statusRequestInFlight = false
            self.daemonRunning = reachable

            let state = status.state
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

    private func updateIcon() {
        guard let button = statusItem.button else { return }

        let issue = !daemonRunning || currentState == .error
        let streaming = currentState.isStreaming && !issue

        // Let AppKit apply the menu-bar appearance tint to the template image;
        // forcing black made the error icon disappear in dark appearance.
        button.wantsLayer = true
        if let layer = button.layer {
            layer.cornerRadius = 5
            layer.masksToBounds = false
            layer.backgroundColor = streaming ? NSColor.systemGreen.cgColor : NSColor.clear.cgColor
        }

        setRecordingDotVisible(currentState == .recording && !issue, on: button)
    }

    private func setRecordingDotVisible(_ visible: Bool, on button: NSStatusBarButton) {
        button.wantsLayer = true
        guard let layer = button.layer else { return }

        if !visible {
            layer.sublayers?.removeAll { $0.name == Self.recordingDotLayerName }
            return
        }

        let dotSize: CGFloat = 6
        let margin: CGFloat = 2
        let dot = layer.sublayers?.first { $0.name == Self.recordingDotLayerName } ?? CALayer()
        dot.name = Self.recordingDotLayerName
        dot.backgroundColor = NSColor.systemOrange.cgColor
        dot.cornerRadius = dotSize / 2
        dot.masksToBounds = true
        dot.frame = CGRect(
            x: max(margin, button.bounds.maxX - dotSize - margin),
            y: margin,
            width: dotSize,
            height: dotSize
        )

        if dot.superlayer == nil {
            layer.addSublayer(dot)
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
        window.setFrameAutosaveName("SotFSystemwideConfiguration")
        window.isRestorable = true
        if !window.setFrameUsingName("SotFSystemwideConfiguration") {
            window.center()
        }
        window.minSize = NSSize(width: 800, height: 500)

        // IMPORTANT: Don't release window when closed, just hide it
        // This prevents the app from quitting when the window is closed
        window.isReleasedWhenClosed = false

        let contentView = ConfigurationView(
            client: client,
            onClose: { [weak window] in
                window?.close()
            },
            onRestartDaemon: { [weak self] in
                self?.daemonManager.restartDaemon()
            },
            onViewDaemonLog: { [weak self] in
                self?.daemonManager.openDaemonLog()
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
    let truePeakDbtp: Double
    let clipLatched: Bool
    let onClearClip: () -> Void

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

            HStack(spacing: 4) {
                Text("TP")
                    .font(.system(size: 9, weight: .semibold))
                    .foregroundColor(.secondary)
                Text(formatTruePeak(truePeakDbtp))
                    .font(.system(size: 9, design: .monospaced))
                    .foregroundColor(clipLatched ? .red : .primary)
                if clipLatched {
                    Button("CLIP", action: onClearClip)
                        .font(.system(size: 8, weight: .bold))
                        .buttonStyle(.borderless)
                        .foregroundColor(.red)
                        .help("Clear latched clip indicator")
                }
            }
            .padding(.horizontal, 5)
            .padding(.bottom, 4)
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

    private func formatTruePeak(_ value: Double) -> String {
        guard value.isFinite, value > -60 else { return "-∞" }
        return String(format: "%.1f", value)
    }
}

// MARK: - Configuration View (SwiftUI)

struct ConfigurationView: View {
    let client: AudioEngineClient
    let onClose: () -> Void
    let onRestartDaemon: () -> Void
    let onViewDaemonLog: () -> Void

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
    @State private var programmaticInputChannelSync = false
    @State private var programmaticOutputChannelSync = false

    // Error handling
    @State private var showingError = false
    @State private var errorMessage = ""

    // Level metering
    @State private var inputPeaks: [Double] = [0.0, 0.0]
    @State private var outputPeaks: [Double] = [0.0, 0.0]
    @State private var inputPeakHolds: [Double] = [0.0, 0.0]
    @State private var outputPeakHolds: [Double] = [0.0, 0.0]
    @State private var inputMomentaryLufs: Double = -60.0
    @State private var inputShortTermLufs: Double = -60.0
    @State private var outputMomentaryLufs: Double = -60.0
    @State private var outputShortTermLufs: Double = -60.0
    @State private var inputTruePeakDbtp: Double = -60.0
    @State private var outputTruePeakDbtp: Double = -60.0
    @State private var inputClipLatched = false
    @State private var outputClipLatched = false
    @State private var meteringTimer: Timer? = nil
    @State private var meteringRequestInFlight = false
    @State private var loadingDevices = false
    @State private var deviceRecoveryTimer: Timer? = nil
    @State private var daemonStatusTimer: Timer? = nil
    @State private var daemonStatusRequestInFlight = false
    @State private var reconnectDelay: TimeInterval = 1
    @State private var nextReconnectAttempt = Date.distantPast
    @State private var volumeUpdateWorkItem: DispatchWorkItem?
    @State private var lastDaemonSelectedDevice: String? = nil
    @State private var programmaticDeviceSelection: String? = nil
    @State private var encryptionToggleGuard = EncryptionToggleGuard()
    @State private var daemonReachable = false

    // Encryption state
    @State private var encryptionEnabled: Bool = true
    @State private var encryptionFingerprint: String = ""
    @State private var encryptionError: String? = nil
    @State private var pluginRackRefreshToken = 0
    @State private var loadingPluginConfiguration = false
    @State private var savingPluginConfiguration = false

    // HAL Configuration state
    @State private var halConfig: AudioEngineClient.HalConfigData = AudioEngineClient.HalConfigData()
    @State private var selectedSampleRate: UInt32 = 48000
    @State private var selectedBufferFrames: UInt32 = 512
    @State private var halConfigError: String? = nil
    @State private var programmaticSampleRateSync = false
    @State private var programmaticBufferFramesSync = false

    let channelOptions = Array(1...32)
    let sampleRateOptions: [UInt32] = [44100, 48000, 96000]
    let bufferFramesOptions: [UInt32] = [128, 256, 512, 1024, 2048]

    private var selectedOutputDevice: AudioEngineClient.AudioDevice? {
        physicalOutputDevices.first { $0.name == selectedDevice }
    }

    private var selectedOutputDeviceChannelLimit: Int? {
        guard let channels = selectedOutputDevice?.channels, channels > 0 else {
            return nil
        }
        return min(max(channels, 1), 32)
    }

    private var outputChannelOptions: [Int] {
        Array(1...(selectedOutputDeviceChannelLimit ?? 32))
    }

    var body: some View {
        HStack(spacing: 0) {
            // Left level meter (input monitor)
            LevelMeterView(
                title: "Monitor In",
                channelPeaks: inputPeaks,
                peakHolds: inputPeakHolds,
                channelLabels: channelLabels(for: inputPeaks.count),
                momentaryLufs: inputMomentaryLufs,
                shortTermLufs: inputShortTermLufs,
                truePeakDbtp: inputTruePeakDbtp,
                clipLatched: inputClipLatched,
                onClearClip: { inputClipLatched = false }
            )
            .frame(width: meterWidth(for: inputPeaks.count))
            .padding(.leading, 8)

            // Main content with scroll
            VStack(spacing: 0) {
                if !daemonReachable {
                    HStack(spacing: 8) {
                        Image(systemName: "wifi.exclamationmark")
                            .foregroundColor(.orange)
                        Text("Daemon not running — controls are disabled while reconnecting")
                            .font(.callout.weight(.medium))
                        Spacer()
                        Button("Restart") {
                            onRestartDaemon()
                            updateDaemonStatus()
                        }
                        .buttonStyle(.borderless)
                        Button("View Log") {
                            onViewDaemonLog()
                        }
                        .buttonStyle(.borderless)
                        ProgressView()
                            .controlSize(.small)
                    }
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(Color.orange.opacity(0.12))
                }

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
                        .onChange(of: halInputChannels) { _, newValue in
                            syncMeterArrays(inputChannels: newValue)
                            if programmaticInputChannelSync {
                                programmaticInputChannelSync = false
                                return
                            }
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
                    HStack {
                        Text("Output Device:")
                            .font(.headline)

                        Picker("Device", selection: $selectedDevice) {
                            if physicalOutputDevices.isEmpty {
                                Text(outputDevicePlaceholderText).tag("")
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
                            if programmaticDeviceSelection == newDevice {
                                programmaticDeviceSelection = nil
                                syncOutputChannelsToSelectedDevice(applyChange: false)
                                return
                            }
                            guard !isVirtualDevice(newDevice) else {
                                errorMessage = "Virtual audio devices cannot be used as Systemwide speaker output. Select hardware speakers/headphones here, and select SotF Virtual Audio in macOS Sound Output."
                                showingError = true
                                loadDevices()
                                return
                            }
                            client.sendCommandAsync(["command": "set_device", "device": newDevice]) { response in
                                if response?.success == true {
                                    syncOutputChannelsToSelectedDevice(applyChange: true)
                                } else {
                                    errorMessage = response?.error ?? "Failed to set output device: \(newDevice)"
                                    showingError = true
                                    if let previous = lastDaemonSelectedDevice {
                                        programmaticDeviceSelection = previous
                                        selectedDevice = previous
                                    }
                                    loadDevices()
                                }
                            }
                        }

                        Button(action: {
                            loadDevices()
                        }) {
                            Image(systemName: loadingDevices ? "hourglass" : "arrow.clockwise")
                        }
                        .buttonStyle(.borderless)
                        .disabled(loadingDevices)
                        .help("Refresh output devices")
                    }
                    .onAppear {
                        loadDevices()
                    }

                    Divider()

                    HStack {
                        Text("Output Channels:")
                            .font(.headline)

                        Picker("", selection: $halOutputChannels) {
                            ForEach(outputChannelOptions, id: \.self) { count in
                                Text("\(count) channel\(count == 1 ? "" : "s")").tag(count)
                            }
                        }
                        .pickerStyle(.menu)
                        .frame(width: 150)
                        .onChange(of: halOutputChannels) { _, newValue in
                            syncMeterArrays(outputChannels: min(max(newValue, 1), 32))
                            if programmaticOutputChannelSync {
                                programmaticOutputChannelSync = false
                                return
                            }
                            applyHALConfiguration()
                        }

                        Spacer()

                        HStack(spacing: 4) {
                            Image(systemName: "info.circle")
                                .foregroundColor(.blue)
                            Text(outputChannelsHelpText)
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                    }

                    Divider()

                    HStack {
                        Text("Volume:")
                        Slider(value: $volume, in: 0...1)
                            .onChange(of: volume) { _, newVolume in
                                volumeUpdateWorkItem?.cancel()
                                let work = DispatchWorkItem {
                                    client.sendCommandAsync(["command": "set_volume", "volume": newVolume]) { response in
                                        if response?.success != true {
                                            errorMessage = response?.error ?? "Failed to set volume"
                                            showingError = true
                                        }
                                    }
                                }
                                volumeUpdateWorkItem = work
                                DispatchQueue.main.asyncAfter(deadline: .now() + 0.05, execute: work)
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
                        Button {
                            loadPluginConfig()
                        } label: {
                            if loadingPluginConfiguration {
                                HStack(spacing: 6) {
                                    ProgressView()
                                        .controlSize(.small)
                                    Text("Loading…")
                                }
                            } else {
                                Text("Load Configuration...")
                            }
                        }
                        .disabled(loadingPluginConfiguration || savingPluginConfiguration)

                        Button {
                            savePluginConfig()
                        } label: {
                            if savingPluginConfiguration {
                                HStack(spacing: 6) {
                                    ProgressView()
                                        .controlSize(.small)
                                    Text("Saving…")
                                }
                            } else {
                                Text("Save Configuration...")
                            }
                        }
                        .disabled(loadingPluginConfiguration || savingPluginConfiguration)

                        Spacer()
                    }

                    Divider()

                    PluginRackView(
                        client: client,
                        outputChannels: halOutputChannels,
                        availableOutputChannels: selectedOutputDeviceChannelLimit,
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
                                if encryptionToggleGuard.consumeProgrammaticChange() {
                                    return
                                }
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
                            Text(halConfig.active ? "HAL Stream Active" : "HAL Stream Idle")
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
                            if programmaticSampleRateSync {
                                programmaticSampleRateSync = false
                                return
                            }
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
                            if programmaticBufferFramesSync {
                                programmaticBufferFramesSync = false
                                return
                            }
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
                    .disabled(!daemonReachable)
                }  // End of ScrollView

                // Status bar (fixed at bottom, not scrollable)
                Divider()
                HStack {
                    Image(systemName: "circle.fill")
                        .foregroundColor(daemonReachable ? .green : .secondary)
                    Text(daemonReachable
                        ? "Connected to audio engine | Source: \(selectedSource.rawValue) | \(halInputChannels)ch in → \(halOutputChannels)ch out"
                        : "Daemon not running — retrying… | Source: \(selectedSource.rawValue)")
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
                momentaryLufs: outputMomentaryLufs,
                shortTermLufs: outputShortTermLufs,
                truePeakDbtp: outputTruePeakDbtp,
                clipLatched: outputClipLatched,
                onClearClip: { outputClipLatched = false }
            )
            .frame(width: meterWidth(for: outputPeaks.count))
            .padding(.trailing, 8)
        }  // End of HStack
        .frame(minWidth: 820, minHeight: 600)
        .onAppear {
            loadDevices()
            updateDaemonStatus()
            startDaemonStatusTimer()
            startMeteringTimer()
        }
        .onDisappear {
            stopDaemonStatusTimer()
            stopMeteringTimer()
            stopDeviceRecoveryPolling()
            volumeUpdateWorkItem?.cancel()
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

    private func startDaemonStatusTimer() {
        guard daemonStatusTimer == nil else { return }
        daemonStatusTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { _ in
            updateDaemonStatus()
        }
    }

    private func stopDaemonStatusTimer() {
        daemonStatusTimer?.invalidate()
        daemonStatusTimer = nil
    }

    private func updateDaemonStatus() {
        guard daemonReachable || Date() >= nextReconnectAttempt else { return }
        guard !daemonStatusRequestInFlight else { return }
        daemonStatusRequestInFlight = true

        AudioEngineClient.pollStatus { status, reachable in
            daemonStatusRequestInFlight = false
            daemonReachable = reachable
            if reachable {
                reconnectDelay = 1
                nextReconnectAttempt = .distantPast
            } else {
                nextReconnectAttempt = Date().addingTimeInterval(reconnectDelay)
                reconnectDelay = min(reconnectDelay * 2, 16)
            }
            applyDaemonStatus(status)
        }
    }

    private func applyDaemonStatus(_ status: AudioEngineClient.Status) {
        if let inputChannels = status.inputChannels, inputChannels > 0, inputChannels != halInputChannels {
            programmaticInputChannelSync = true
            halInputChannels = min(max(inputChannels, 1), 32)
            syncMeterArrays(inputChannels: halInputChannels)
        }

        let daemonOutputChannels = status.outputChannels ?? status.channels
        if let channels = daemonOutputChannels, channels > 0, channels != halOutputChannels {
            programmaticOutputChannelSync = true
            halOutputChannels = min(max(channels, 1), 32)
            syncMeterArrays(outputChannels: halOutputChannels)
        }

        guard let daemonDevice = status.selectedDevice,
              !daemonDevice.isEmpty,
              !isVirtualDevice(daemonDevice),
              physicalOutputDevices.contains(where: { $0.name == daemonDevice }) else {
            return
        }

        lastDaemonSelectedDevice = daemonDevice
        guard selectedDevice != daemonDevice else { return }

        programmaticDeviceSelection = daemonDevice
        selectedDevice = daemonDevice
        syncOutputChannelsToSelectedDevice(applyChange: false)
    }

    private func updateMetering() {
        guard daemonReachable else {
            inputPeaks = decayedPeaks(inputPeaks)
            outputPeaks = decayedPeaks(outputPeaks)
            return
        }

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
        AudioEngineClient.pollMetering { metering in
            meteringRequestInFlight = false
            applyMetering(metering)
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

            if let input = metering.input {
                inputMomentaryLufs = input.momentary
                inputShortTermLufs = input.shortTerm
                if let truePeak = input.truePeaksDbtp.filter(\.isFinite).max() {
                    inputTruePeakDbtp = truePeak
                    if truePeak >= 0 {
                        inputClipLatched = true
                    }
                }
            }

            // Output peaks from post-processing monitor
            if let output = metering.output {
                if !output.channelPeaks.isEmpty {
                    nextOutputPeaks = sanitizedPeaks(output.channelPeaks)
                } else {
                    nextOutputPeaks = sanitizedPeaks(Array(repeating: output.peak, count: max(outputPeaks.count, 1)))
                }
                outputMomentaryLufs = output.momentary
                outputShortTermLufs = output.shortTerm
                if let truePeak = output.truePeaksDbtp.filter(\.isFinite).max() {
                    outputTruePeakDbtp = truePeak
                    if truePeak >= 0 {
                        outputClipLatched = true
                    }
                }
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

    private func syncMeterArrays(inputChannels: Int? = nil, outputChannels: Int? = nil) {
        if let inputChannels {
            inputPeaks = resizedPeaks(inputPeaks, count: inputChannels)
            inputPeakHolds = resizedPeaks(inputPeakHolds, count: inputChannels)
        }
        if let outputChannels {
            outputPeaks = resizedPeaks(outputPeaks, count: outputChannels)
            outputPeakHolds = resizedPeaks(outputPeakHolds, count: outputChannels)
        }
    }

    private func resizedPeaks(_ peaks: [Double], count: Int) -> [Double] {
        let target = min(max(count, 1), 32)
        if peaks.count == target {
            return peaks
        }
        if peaks.count > target {
            return Array(peaks.prefix(target))
        }
        return peaks + Array(repeating: 0.0, count: target - peaks.count)
    }

    private func sanitizedPeaks(_ peaks: [Double]) -> [Double] {
        sanitizeConfigBarPeaks(peaks)
    }

    private func decayedPeaks(_ peaks: [Double]) -> [Double] {
        decayConfigBarPeaks(peaks)
    }

    private func updatedPeakHolds(previous: [Double], current: [Double]) -> [Double] {
        updateConfigBarPeakHolds(previous: previous, current: current)
    }

    /// Virtual device patterns that should not be used as speaker output.
    /// Check if a device name matches a virtual device pattern
    private func isVirtualDevice(_ name: String) -> Bool {
        isConfigBarVirtualDevice(name)
    }

    private var physicalOutputDevices: [AudioEngineClient.AudioDevice] {
        devices.filter { !isVirtualDevice($0.name) }
    }

    private var outputChannelsHelpText: String {
        if let limit = selectedOutputDeviceChannelLimit {
            return "Selected interface supports \(limit) channel\(limit == 1 ? "" : "s")"
        }
        return "2=stereo, 6=5.1, 10=5.1.4, up to 32"
    }

    private var outputDevicePlaceholderText: String {
        if loadingDevices {
            return "Refreshing hardware output devices..."
        }
        if deviceRecoveryTimer != nil {
            return "Waiting for CoreAudio hardware devices..."
        }
        return "No hardware output devices"
    }

    private func startDeviceRecoveryPolling() {
        guard deviceRecoveryTimer == nil else { return }
        deviceRecoveryTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { _ in
            loadDevices()
        }
    }

    private func stopDeviceRecoveryPolling() {
        deviceRecoveryTimer?.invalidate()
        deviceRecoveryTimer = nil
    }

    private func loadDevices() {
        guard !loadingDevices else { return }
        loadingDevices = true

        DispatchQueue.global(qos: .utility).async {
            var loadedDevices = AudioEngineClient().listDevices()
            let status = AudioEngineClient().getStatus()
            if loadedDevices.isEmpty {
                loadedDevices = detectOutputDevicesViaCoreAudio()
            }

            DispatchQueue.main.async {
                loadingDevices = false
                devices = loadedDevices
                applyLoadedDevices(daemonSelectedDevice: status.selectedDevice)
                applyDaemonStatus(status)
            }
        }
    }

    private func applyLoadedDevices(daemonSelectedDevice: String? = nil) {
        // Filter out virtual devices for output selection
        let physicalDevices = physicalOutputDevices
        let previousDevice = selectedDevice
        var selectedFromDaemon = false

        if physicalDevices.isEmpty {
            selectedDevice = ""
            startDeviceRecoveryPolling()
            detectAvailableSources()
            return
        }

        stopDeviceRecoveryPolling()

        if let daemonSelectedDevice,
           !daemonSelectedDevice.isEmpty,
           !isVirtualDevice(daemonSelectedDevice),
           physicalDevices.contains(where: { $0.name == daemonSelectedDevice }) {
            programmaticDeviceSelection = daemonSelectedDevice
            selectedDevice = daemonSelectedDevice
            lastDaemonSelectedDevice = daemonSelectedDevice
            selectedFromDaemon = true
        } else if let lastDaemonSelectedDevice,
                  physicalDevices.contains(where: { $0.name == lastDaemonSelectedDevice }) {
            programmaticDeviceSelection = lastDaemonSelectedDevice
            selectedDevice = lastDaemonSelectedDevice
            selectedFromDaemon = true
        } else if let physicalDefault = physicalDevices.first(where: { $0.is_default }) {
            selectedDevice = physicalDefault.name
        } else if !previousDevice.isEmpty,
                  physicalDevices.contains(where: { $0.name == previousDevice }) {
            selectedDevice = previousDevice
        } else if let firstPhysical = physicalDevices.first {
            // Use the first physical device
            selectedDevice = firstPhysical.name
        } else {
            selectedDevice = ""
        }

        syncOutputChannelsToSelectedDevice(applyChange: !selectedFromDaemon)

        // Also detect available audio sources
        detectAvailableSources()
    }

    private func syncOutputChannelsToSelectedDevice(applyChange: Bool) {
        guard let limit = selectedOutputDeviceChannelLimit else { return }
        guard halOutputChannels > limit else { return }

        if !applyChange {
            programmaticOutputChannelSync = true
        }
        halOutputChannels = limit
        if applyChange {
            applyHALConfiguration()
        }
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
        guard halInputChannels >= 1 && halInputChannels <= 32 else {
            errorMessage = "Invalid input channel count: \(halInputChannels). Must be between 1 and 32."
            showingError = true
            return
        }

        guard halOutputChannels >= 1 && halOutputChannels <= 32 else {
            errorMessage = "Invalid output channel count: \(halOutputChannels). Must be between 1 and 32."
            showingError = true
            return
        }

        let command: [String: Any] = [
            "command": "set_pipeline_channels",
            "input_channels": halInputChannels,
            "output_channels": halOutputChannels
        ]

        client.sendCommandAsync(command) { response in
            if response?.success == true {
                print("✅ HAL configuration applied: \(halOutputChannels)ch out")
            } else {
                errorMessage = response?.error ?? "Failed to communicate with daemon. Please ensure the daemon is running."
                showingError = true
            }
        }
    }

    private func loadPluginConfig() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.json]
        panel.allowsMultipleSelection = false
        panel.message = "Select plugin configuration file"

        if panel.runModal() == .OK, let url = panel.url {
            loadingPluginConfiguration = true
            do {
                let data = try Data(contentsOf: url)
                let json = try JSONSerialization.jsonObject(with: data)

                let command: [String: Any] = [
                    "command": "load_plugin_artifact",
                    "artifact": json
                ]

                client.sendCommandAsync(command) { response in
                    loadingPluginConfiguration = false
                    if response?.success == true {
                        print("✅ Plugin configuration loaded from: \(url.path)")
                        pluginRackRefreshToken += 1
                    } else {
                        errorMessage = response?.error ?? "Failed to apply plugin configuration"
                        showingError = true
                    }
                }
            } catch {
                loadingPluginConfiguration = false
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
            savingPluginConfiguration = true
            DispatchQueue.global(qos: .utility).async {
                let saveClient = AudioEngineClient()
                let pipeline = saveClient.getPluginPipeline()
                let available = pipeline?.graph == nil
                    ? (saveClient.getAvailablePlugins() ?? [])
                    : []
                DispatchQueue.main.async {
                    guard let pipeline else {
                        savingPluginConfiguration = false
                        errorMessage = "Failed to retrieve current plugin list from daemon"
                        showingError = true
                        return
                    }
                    do {
                        let artifact: Any
                        if let graph = pipeline.graph {
                            artifact = ["graph": graph.artifact]
                        } else {
                            guard let preset = appGpuiPreset(
                                from: pipeline.plugins,
                                available: available
                            ) else {
                                throw NSError(
                                    domain: "SotFConfigBar",
                                    code: 1,
                                    userInfo: [NSLocalizedDescriptionKey: "The current chain contains a plugin type this configuration format cannot represent."]
                                )
                            }
                            artifact = preset
                        }
                        let data = try JSONSerialization.data(
                            withJSONObject: artifact,
                            options: .prettyPrinted
                        )
                        try data.write(to: url)
                        print("✅ Plugin configuration saved to: \(url.path)")
                    } catch {
                        errorMessage = "Failed to save configuration: \(error.localizedDescription)"
                        showingError = true
                    }
                    savingPluginConfiguration = false
                }
            }
        }
    }

    private func appGpuiPreset(
        from plugins: [[String: Any]],
        available: [AvailablePlugin]
    ) -> [String: Any]? {
        let defaultsByType = Dictionary(uniqueKeysWithValues: available.map { ($0.type_, $0.defaultParameters) })

        let unsupportedTypes = plugins.compactMap { plugin -> String? in
            guard let type = plugin["plugin_type"] as? String else {
                return "<missing plugin_type>"
            }
            guard isSystemPluginType(type) || engineTypeToAppGpuiSettingsVariant[type] != nil else {
                return type
            }
            return nil
        }
        guard unsupportedTypes.isEmpty else {
            print("Cannot save unsupported plugin types: \(unsupportedTypes.joined(separator: ", "))")
            return nil
        }

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

    // MARK: - Encryption Methods

    private func setEncryption(enabled: Bool) {
        encryptionError = nil
        client.sendCommandAsync(["command": "set_encryption", "enabled": enabled]) { response in
            if response?.success == true {
                print("✅ Encryption \(enabled ? "enabled" : "disabled")")
                refreshEncryptionStatus()
            } else {
                encryptionError = response?.error ?? "Failed to \(enabled ? "enable" : "disable") encryption"
                // Revert the toggle state without feeding the failure back
                // through the Toggle's onChange handler.
                encryptionToggleGuard.markProgrammaticChange()
                encryptionEnabled = !enabled
            }
        }
    }

    private func rotateEncryptionKey() {
        encryptionError = nil

        client.sendCommandAsync(["command": "rotate_encryption_key"]) { response in
            if response?.success == true {
                print("✅ Encryption key rotated")
                refreshEncryptionStatus()
            } else {
                encryptionError = response?.error ?? "Failed to rotate encryption key"
            }
        }
    }

    private func refreshEncryptionStatus() {
        DispatchQueue.global(qos: .utility).async {
            let status = AudioEngineClient().getEncryptionStatus()

            DispatchQueue.main.async {
                if let status = status {
                    if encryptionEnabled != status.enabled {
                        encryptionToggleGuard.markProgrammaticChange()
                        encryptionEnabled = status.enabled
                    }
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
                    if config.actualSampleRate != 0 && selectedSampleRate != config.actualSampleRate {
                        programmaticSampleRateSync = true
                        selectedSampleRate = config.actualSampleRate
                    }
                    if config.actualBufferFrames != 0 && selectedBufferFrames != config.actualBufferFrames {
                        programmaticBufferFramesSync = true
                        selectedBufferFrames = config.actualBufferFrames
                    }
                    if config.channelCount != 0 && halInputChannels != Int(config.channelCount) {
                        programmaticInputChannelSync = true
                        halInputChannels = Int(config.channelCount)
                        syncMeterArrays(inputChannels: Int(config.channelCount))
                    }
                } else {
                    halConfigError = "Failed to get HAL config (daemon may not be running)"
                }
            }
        }
    }

    private func setSampleRate(_ rate: UInt32) {
        halConfigError = nil
        client.sendCommandAsync(["command": "set_sample_rate", "rate": rate]) { response in
            if response?.success == true {
                print("Sample rate set to \(rate) Hz")
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                    refreshHalConfig()
                }
            } else {
                halConfigError = response?.error ?? "Failed to set sample rate"
                programmaticSampleRateSync = true
                selectedSampleRate = halConfig.actualSampleRate != 0 ? halConfig.actualSampleRate : 48000
            }
        }
    }

    private func setBufferFrames(_ frames: UInt32) {
        halConfigError = nil
        client.sendCommandAsync(["command": "set_buffer_frames", "frames": frames]) { response in
            if response?.success == true {
                print("Buffer frames set to \(frames)")
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                    refreshHalConfig()
                }
            } else {
                halConfigError = response?.error ?? "Failed to set buffer frames"
                programmaticBufferFramesSync = true
                selectedBufferFrames = halConfig.actualBufferFrames != 0 ? halConfig.actualBufferFrames : 512
            }
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
        // Hide dock icon (menu bar only app)
        NSApp.setActivationPolicy(.accessory)

        // Create status bar controller (which starts the daemon automatically)
        statusBarController = StatusBarController()

        print("SotF Systemwide menu bar app started")
    }

    func applicationWillTerminate(_ notification: Notification) {
        statusBarController?.stopMonitoring()
        statusBarController?.stopDaemon()
        print("SotF Systemwide menu bar app terminated")
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
