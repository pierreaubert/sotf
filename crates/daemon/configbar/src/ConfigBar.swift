#!/usr/bin/swift
//
// AutoEQ Menu Bar Application
//
// A macOS menu bar app that controls the AutoEQ audio engine with:
// - Color-coded speaker icon (grey/green/red) based on audio activity
// - Configuration window for audio interfaces and plugin chains
// - Energy optimization (stops engine after 3s of silence)
// - Integration with src-audio daemon via Unix socket

import SwiftUI
import Cocoa
import UserNotifications
import WebKit
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

/// Client for communicating with the sotf_daemon via Unix socket
class AudioEngineClient {
    private let socketPath = "/tmp/autoeq_audio.sock"
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
            }
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

            // Give the daemon a moment to process
            usleep(10000) // 10ms

            // Read response (read until newline or buffer full)
            var responseData = Data()
            var buffer = [UInt8](repeating: 0, count: 4096)
            let bufferCount = buffer.count

            let bytesRead = buffer.withUnsafeMutableBufferPointer { bufferPtr in
                Darwin.recv(socketFD, bufferPtr.baseAddress, bufferCount, 0)
            }

            guard bytesRead > 0 else {
                print("Failed to read response: \(String(cString: strerror(errno)))")
                return nil
            }

            responseData.append(contentsOf: buffer[0..<bytesRead])

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
}

// MARK: - Status Bar Controller

@MainActor
class StatusBarController: ObservableObject {
    private var statusItem: NSStatusItem
    @Published var currentState: AudioEngineClient.AudioState = .idle
    @Published var showingWindow = false

    private let client = AudioEngineClient()
    private var monitorTimer: Timer?

    init() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)

        if let button = statusItem.button {
            // Use SF Symbol for speaker
            let image = NSImage(systemSymbolName: "speaker.wave.2.fill",
                              accessibilityDescription: "AutoEQ")!
            image.isTemplate = true
            button.image = image
            button.target = self
            button.action = #selector(statusBarButtonClicked)
            button.toolTip = "AutoEQ Audio Engine"
        }

        // Connect to daemon
        _ = client.connect()

        // Start monitoring
        startMonitoring()

        updateIcon()
    }

    @objc func statusBarButtonClicked() {
        showingWindow.toggle()

        if showingWindow {
            showConfigWindow()
        }
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

    private func updateStatus() {
        let (state, volume, muted) = client.getStatus()

        if currentState != state {
            currentState = state
            updateIcon()
        }
    }

    private func updateIcon() {
        guard let button = statusItem.button else { return }

        // Tint the icon based on state
        button.contentTintColor = currentState.iconColor
    }

    private func showConfigWindow() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 800, height: 600),
            styleMask: [.titled, .closable, .resizable],
            backing: .buffered,
            defer: false
        )

        window.title = "AutoEQ Configuration"
        window.center()

        let contentView = ConfigurationView(
            client: client,
            onClose: { [weak self] in
                self?.showingWindow = false
            }
        )

        window.contentView = NSHostingView(rootView: contentView)
        window.makeKeyAndOrderFront(nil)

        // Keep window open
        NSApp.activate(ignoringOtherApps: true)
    }
}

// MARK: - Configuration View (SwiftUI)

struct ConfigurationView: View {
    let client: AudioEngineClient
    let onClose: () -> Void

    @State private var devices: [AudioEngineClient.AudioDevice] = []
    @State private var selectedDevice: String = ""
    @State private var volume: Float = 1.0
    @State private var showingPluginConfig = false

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

    let channelOptions = Array(1...16)

    var body: some View {
        VStack(spacing: 20) {
            // Header
            HStack {
                Text("AutoEQ Audio Configuration")
                    .font(.title)
                Spacer()
                Button("Close") {
                    onClose()
                }
            }
            .padding()

            Divider()

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
                        .onChange(of: halInputChannels) { _ in
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
                        ForEach(devices, id: \.name) { device in
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
                    .onChange(of: selectedDevice) { newDevice in
                        _ = client.setDevice(newDevice)
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
                        .onChange(of: halOutputChannels) { _ in
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
                            .onChange(of: volume) { newVolume in
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

                        Button("Edit Plugins") {
                            showingPluginConfig = true
                        }
                    }

                    if showingPluginConfig {
                        Divider()
                        PluginHostView()
                    }
                }
                .padding()
            }

            Spacer()

            // Status
            HStack {
                Image(systemName: "circle.fill")
                    .foregroundColor(.green)
                Text("Connected to audio engine | Source: \(selectedSource.rawValue) | \(halInputChannels)ch in → \(halOutputChannels)ch out")
                    .foregroundColor(.secondary)
            }
            .padding()
        }
        .frame(minWidth: 700, minHeight: 600)
        .onAppear {
            loadDevices()
        }
        .alert("Configuration Error", isPresented: $showingError) {
            Button("OK", role: .cancel) { }
        } message: {
            Text(errorMessage)
        }
    }

    private func loadDevices() {
        devices = client.listDevices()

        // Select the default device if available, otherwise first device
        if let defaultDevice = devices.first(where: { $0.is_default }) {
            selectedDevice = defaultDevice.name
        } else if let firstDevice = devices.first {
            selectedDevice = firstDevice.name
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

        // Also check using Core Audio API for more thorough detection
        detectAudioDevicesViaCoreAudio()

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

    /// Get the name of an audio device by its ID
    private func getDeviceName(deviceID: AudioDeviceID) -> String? {
        var propertyAddress = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyDeviceNameCFString,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )

        var name: CFString?
        var dataSize = UInt32(MemoryLayout<CFString?>.size)

        let status = AudioObjectGetPropertyData(
            deviceID,
            &propertyAddress,
            0,
            nil,
            &dataSize,
            &name
        )

        if status == noErr, let deviceName = name as String? {
            return deviceName
        }
        return nil
    }

    private func applyHALConfiguration() {
        // Validate channel configuration
        guard halInputChannels >= 1 && halInputChannels <= 16 else {
            errorMessage = "Invalid input channel count: \(halInputChannels). Must be between 1 and 16."
            showingError = true
            return
        }

        guard halOutputChannels >= 1 && halOutputChannels <= 16 else {
            errorMessage = "Invalid output channel count: \(halOutputChannels). Must be between 1 and 16."
            showingError = true
            return
        }

        // Build HAL plugin chain with configured channels
        let plugins: [[String: Any]] = [
            [
                "plugin_type": "hal_input",
                "parameters": ["channels": halInputChannels]
            ],
            [
                "plugin_type": "hal_output",
                "parameters": ["channels": halOutputChannels]
            ]
        ]

        let command: [String: Any] = [
            "command": "load_plugins",
            "plugins": plugins
        ]

        guard let response = client.sendCommand(command) else {
            errorMessage = "Failed to communicate with daemon. Please ensure the daemon is running."
            showingError = true
            return
        }

        if response.success {
            print("✅ HAL configuration applied: \(halInputChannels)ch in → \(halOutputChannels)ch out")
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
            print("Loading config from: \(url.path)")
            // TODO: Load and apply plugin configuration
        }
    }

    private func savePluginConfig() {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.json]
        panel.nameFieldStringValue = "autoeq_plugins.json"
        panel.message = "Save plugin configuration"

        if panel.runModal() == .OK, let url = panel.url {
            print("Saving config to: \(url.path)")
            // TODO: Save current plugin configuration
        }
    }
}

// MARK: - Plugin Host View (WebView for TypeScript UI)

struct PluginHostView: NSViewRepresentable {
    func makeNSView(context: Context) -> WKWebView {
        let webView = WKWebView()

        // Load the TypeScript UI
        // In production, this would load from src-ui-frontend
        let htmlPath = "/Users/pierre/src/autoEQ/src-ui-frontend/index.html"

        if let url = URL(string: "file://\(htmlPath)") {
            webView.loadFileURL(url, allowingReadAccessTo: URL(fileURLWithPath: "/Users/pierre/src/autoEQ/src-ui-frontend"))
        }

        return webView
    }

    func updateNSView(_ nsView: WKWebView, context: Context) {
        // Update if needed
    }
}

// MARK: - App Delegate

class AppDelegate: NSObject, NSApplicationDelegate {
    var statusBarController: StatusBarController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Hide dock icon (menu bar only app)
        NSApp.setActivationPolicy(.accessory)

        // Create status bar controller
        Task { @MainActor in
            statusBarController = StatusBarController()
        }

        // Show startup notification
        NotificationManager.shared.showNotification(
            title: "AutoEQ Started",
            body: "Audio engine control ready"
        )

        print("AutoEQ menu bar app started")
    }

    func applicationWillTerminate(_ notification: Notification) {
        Task { @MainActor in
            statusBarController?.stopMonitoring()
        }
        print("AutoEQ menu bar app terminated")
    }
}

// MARK: - Notification Manager

class NotificationManager: NSObject, UNUserNotificationCenterDelegate {
    static let shared = NotificationManager()

    private override init() {
        super.init()
        setupNotifications()
    }

    private func setupNotifications() {
        let center = UNUserNotificationCenter.current()
        center.delegate = self

        center.requestAuthorization(options: [.alert, .sound, .badge]) { granted, error in
            if granted {
                print("Notification permission granted")
            } else if let error = error {
                print("Notification permission error: \(error)")
            }
        }
    }

    func showNotification(title: String, body: String, sound: Bool = true) {
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

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.run()
