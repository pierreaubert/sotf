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

    /// Try secure path first, then legacy path
    private var socketPath: String {
        let securePath = Self.getSecureSocketPath()
        if FileManager.default.fileExists(atPath: securePath) {
            return securePath
        }
        // Fall back to legacy path (might be a symlink to secure path)
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
        loudness.momentary = data["momentary"]?.value as? Double ?? -60.0
        loudness.shortTerm = data["short_term"]?.value as? Double ?? -60.0
        loudness.integrated = data["integrated"]?.value as? Double ?? -60.0
        loudness.peak = data["peak"]?.value as? Double ?? 0.0

        if let peaks = data["channel_peaks"]?.value as? [Any] {
            loudness.channelPeaks = peaks.compactMap { $0 as? Double }
        }
        if let truePeaks = data["true_peaks_dbtp"]?.value as? [Any] {
            loudness.truePeaksDbtp = truePeaks.compactMap { $0 as? Double }
        }
        if let correlation = data["correlation_lr"]?.value as? Double {
            loudness.correlationLR = correlation
        }

        return loudness
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
        // Look for daemon in several locations (note: binary is named sotf-daemon with hyphen)
        let possiblePaths = [
            // In app bundle's Helpers directory
            Bundle.main.bundlePath + "/Contents/Helpers/sotf-daemon",
            // In same directory as app
            (Bundle.main.bundlePath as NSString).deletingLastPathComponent + "/sotf-daemon",
            // System-wide installation
            "/usr/local/bin/sotf-daemon",
            // Development build (cargo uses underscores)
            FileManager.default.currentDirectoryPath + "/target/release/sotf_daemon"
        ]

        daemonPath = possiblePaths.first { FileManager.default.isExecutableFile(atPath: $0) } ?? possiblePaths[0]
        print("DaemonManager: Using daemon path: \(daemonPath)")
    }

    /// Start the daemon if not already running
    func startDaemon() {
        guard !isShuttingDown else { return }

        // Check if already running
        if let process = daemonProcess, process.isRunning {
            print("DaemonManager: Daemon already running (PID: \(process.processIdentifier))")
            return
        }

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

        // Redirect output to console for debugging
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = pipe

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

            if self.daemonProcess == nil || !self.daemonProcess!.isRunning {
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

    // Daemon management
    private let daemonManager = DaemonManager()
    @Published var daemonRunning = false

    override init() {
        super.init()

        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)

        if let button = statusItem.button {
            // Try to load custom icon from bundle assets
            if let iconImage = loadMenuBarIcon() {
                button.image = iconImage
            } else {
                // Fallback: use SF Symbol
                let config = NSImage.SymbolConfiguration(pointSize: 16, weight: .regular)
                if let image = NSImage(systemSymbolName: "speaker.wave.2.fill",
                                       accessibilityDescription: "SotF")?
                    .withSymbolConfiguration(config) {
                    let templateImage = image.copy() as! NSImage
                    templateImage.isTemplate = true
                    button.image = templateImage
                } else {
                    // Final fallback: use simple text
                    button.title = "♪"
                }
            }
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

    /// Load custom menubar icon from bundle or assets directory
    private func loadMenuBarIcon() -> NSImage? {
        // Standard menubar icon size is 18-22 points
        // We use 22pt which is common for menubar apps
        let iconSize = NSSize(width: 22, height: 22)

        // Try loading from bundle resources first (for packaged app)
        // Try 22pt first, then fall back to 18pt
        for resourceName in ["icon_22", "icon_18"] {
            if let bundleIcon = Bundle.main.image(forResource: resourceName) {
                bundleIcon.isTemplate = true
                bundleIcon.size = iconSize
                return bundleIcon
            }
        }

        // Try loading from assets directory (for development)
        let assetPaths = [
            // Relative to executable (when running from build)
            "../assets/icon_22@2x.png",
            "../assets/icon_22.png",
            "../assets/icon_18@2x.png",
            // Relative to source (when running via swift directly)
            "assets/icon_22@2x.png",
            "assets/icon_22.png",
            "assets/icon_18@2x.png",
            // Absolute paths for development
            "\(FileManager.default.currentDirectoryPath)/assets/icon_22@2x.png",
        ]

        for path in assetPaths {
            if FileManager.default.fileExists(atPath: path),
               let image = NSImage(contentsOfFile: path) {
                image.isTemplate = true
                image.size = iconSize
                return image
            }
        }

        // Try loading from script directory
        let scriptPath = CommandLine.arguments[0]
        if let scriptDir = URL(string: scriptPath)?.deletingLastPathComponent().path {
            for iconName in ["icon_22@2x.png", "icon_22.png", "icon_18@2x.png"] {
                let iconPath = "\(scriptDir)/assets/\(iconName)"
                if FileManager.default.fileExists(atPath: iconPath),
                   let image = NSImage(contentsOfFile: iconPath) {
                    image.isTemplate = true
                    image.size = iconSize
                    return image
                }
            }
        }

        return nil
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
        let (state, _, _) = client.getStatus()

        if currentState != state {
            currentState = state
            updateIcon()

            // Update menu item
            if let menu = statusItem.menu, let statusItem = menu.item(withTag: 100) {
                statusItem.title = "Status: \(state.rawValue)"
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

/// Single channel level meter bar with optional LUFS markers
struct LevelMeterBar: View {
    let level: Double  // Linear peak value (0.0 to 1.0+)
    let momentaryLufs: Double?  // Momentary LUFS (-60 to 0)
    let shortTermLufs: Double?  // Short-term LUFS (-60 to 0)
    let width: CGFloat

    init(level: Double, momentaryLufs: Double? = nil, shortTermLufs: Double? = nil, width: CGFloat = 16) {
        self.level = level
        self.momentaryLufs = momentaryLufs
        self.shortTermLufs = shortTermLufs
        self.width = width
    }

    var body: some View {
        GeometryReader { geometry in
            let height = geometry.size.height
            // Convert linear to dB
            let db = level > 0.00001 ? 20.0 * log10(level) : -60.0
            let fillRatio = dbToPosition(db)
            let fillHeight = CGFloat(fillRatio) * height

            ZStack(alignment: .bottom) {
                // Background
                RoundedRectangle(cornerRadius: 2)
                    .fill(Color.black.opacity(0.3))

                // Meter fill with gradient colors based on level
                if fillRatio > 0 {
                    let color: Color = fillRatio > 0.9 ? .red : (fillRatio > 0.6 ? .yellow : .green)
                    RoundedRectangle(cornerRadius: 2)
                        .fill(color)
                        .frame(height: fillHeight)
                }

                // Momentary LUFS marker (cyan, thick)
                if let mLufs = momentaryLufs, mLufs > -60 && !mLufs.isNaN && !mLufs.isInfinite {
                    let mPos = dbToPosition(mLufs)
                    let yOffset = height - (CGFloat(mPos) * height)
                    Rectangle()
                        .fill(Color.cyan)
                        .frame(width: width, height: 3)
                        .position(x: width / 2, y: yOffset)
                }

                // Short-term LUFS marker (blue, thick)
                if let sLufs = shortTermLufs, sLufs > -60 && !sLufs.isNaN && !sLufs.isInfinite {
                    let sPos = dbToPosition(sLufs)
                    let yOffset = height - (CGFloat(sPos) * height)
                    Rectangle()
                        .fill(Color.blue)
                        .frame(width: width, height: 3)
                        .position(x: width / 2, y: yOffset)
                }
            }
        }
        .frame(width: width)
    }
}

/// Level meter group showing peak meters with LUFS markers
struct LevelMeterView: View {
    let title: String
    let channelPeaks: [Double]
    let channelLabels: [String]
    let momentaryLufs: Double
    let shortTermLufs: Double

    var body: some View {
        VStack(spacing: 0) {
            // Title
            Text(title)
                .font(.system(size: 11, weight: .semibold))
                .foregroundColor(.secondary)
                .padding(.top, 8)
                .padding(.bottom, 4)

            // Main meter area - fills available space
            GeometryReader { geometry in
                HStack(spacing: 3) {
                    // dB scale legend
                    VStack(alignment: .trailing, spacing: 0) {
                        Text("0").font(.system(size: 8)).foregroundColor(.secondary)
                        Spacer()
                        Text("-12").font(.system(size: 8)).foregroundColor(.secondary)
                        Spacer()
                        Text("-24").font(.system(size: 8)).foregroundColor(.secondary)
                        Spacer()
                        Text("-60").font(.system(size: 8)).foregroundColor(.secondary)
                    }
                    .frame(width: 18)

                    // Peak meter bars with LUFS markers
                    ForEach(Array(zip(channelPeaks.indices, channelPeaks)), id: \.0) { _, peak in
                        LevelMeterBar(
                            level: peak,
                            momentaryLufs: momentaryLufs,
                            shortTermLufs: shortTermLufs,
                            width: 16
                        )
                    }
                }
            }

            // Channel labels row
            HStack(spacing: 3) {
                Text("")
                    .frame(width: 18)

                ForEach(Array(channelPeaks.indices), id: \.self) { index in
                    let label = index < channelLabels.count ? channelLabels[index] : "\(index + 1)"
                    Text(label)
                        .font(.system(size: 9, weight: .medium))
                        .foregroundColor(.secondary)
                        .frame(width: 16)
                }
            }
            .padding(.top, 4)

            // LUFS legend and values
            VStack(spacing: 2) {
                HStack(spacing: 4) {
                    Rectangle().fill(Color.cyan).frame(width: 12, height: 3)
                    Text("M: \(formatLufs(momentaryLufs))")
                        .font(.system(size: 9, design: .monospaced))
                        .foregroundColor(.cyan)
                }
                HStack(spacing: 4) {
                    Rectangle().fill(Color.blue).frame(width: 12, height: 3)
                    Text("S: \(formatLufs(shortTermLufs))")
                        .font(.system(size: 9, design: .monospaced))
                        .foregroundColor(.blue)
                }
            }
            .padding(.top, 4)
            .padding(.bottom, 8)
        }
        .frame(maxHeight: .infinity)
        .background(Color.black.opacity(0.1))
        .cornerRadius(8)
    }

    private func formatLufs(_ lufs: Double) -> String {
        if lufs.isNaN || lufs.isInfinite || lufs < -60 {
            return "-∞"
        }
        return String(format: "%.1f", lufs)
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

    // Level metering
    @State private var inputPeaks: [Double] = [0.0, 0.0]
    @State private var outputPeaks: [Double] = [0.0, 0.0]
    @State private var momentaryLufs: Double = -60.0
    @State private var shortTermLufs: Double = -60.0
    @State private var meteringTimer: Timer? = nil

    // Encryption state
    @State private var encryptionEnabled: Bool = false
    @State private var encryptionFingerprint: String = ""
    @State private var encryptionError: String? = nil

    let channelOptions = Array(1...16)

    var body: some View {
        HStack(spacing: 0) {
            // Left level meter (Input)
            LevelMeterView(
                title: "Input",
                channelPeaks: inputPeaks,
                channelLabels: inputPeaks.count == 2 ? ["L", "R"] : (1...inputPeaks.count).map { "\($0)" },
                momentaryLufs: momentaryLufs,
                shortTermLufs: shortTermLufs
            )
            .frame(width: 70)
            .padding(.leading, 8)

            // Main content
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
                    .onChange(of: selectedDevice) { _, newDevice in
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

                Spacer()

                // Status
                HStack {
                    Image(systemName: "circle.fill")
                        .foregroundColor(.green)
                    Text("Connected to audio engine | Source: \(selectedSource.rawValue) | \(halInputChannels)ch in → \(halOutputChannels)ch out")
                        .foregroundColor(.secondary)
                }
                .padding()
            }  // End of main VStack

            // Right level meter (Output)
            LevelMeterView(
                title: "Output",
                channelPeaks: outputPeaks,
                channelLabels: outputPeaks.count == 2 ? ["L", "R"] : (1...outputPeaks.count).map { "\($0)" },
                momentaryLufs: momentaryLufs,
                shortTermLufs: shortTermLufs
            )
            .frame(width: 70)
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

    private func startMeteringTimer() {
        meteringTimer = Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { _ in
            updateMetering()
        }
    }

    private func stopMeteringTimer() {
        meteringTimer?.invalidate()
        meteringTimer = nil
    }

    private func updateMetering() {
        if let loudness = client.getLoudness() {
            // Update output peaks from the loudness data
            if !loudness.channelPeaks.isEmpty {
                outputPeaks = loudness.channelPeaks
            }
            // For input, we currently show the same data
            // TODO: Add separate input metering plugin
            inputPeaks = loudness.channelPeaks

            // Update LUFS values
            momentaryLufs = loudness.momentary
            shortTermLufs = loudness.shortTerm
        }
    }

    /// Virtual device patterns that should not be used as output
    private let virtualDevicePatterns = ["SotF", "BlackHole", "Loopback", "Virtual"]

    /// Check if a device name matches a virtual device pattern
    private func isVirtualDevice(_ name: String) -> Bool {
        return virtualDevicePatterns.contains { name.contains($0) }
    }

    private func loadDevices() {
        devices = client.listDevices()

        // Filter out virtual devices for output selection
        let physicalDevices = devices.filter { !isVirtualDevice($0.name) }

        // Prefer a physical device as output, avoiding virtual devices (HAL, BlackHole)
        if let physicalDefault = physicalDevices.first(where: { $0.is_default }) {
            // Use the physical default device
            selectedDevice = physicalDefault.name
        } else if let firstPhysical = physicalDevices.first {
            // Use the first physical device
            selectedDevice = firstPhysical.name
        } else if let defaultDevice = devices.first(where: { $0.is_default }) {
            // Fallback to system default if no physical devices found
            selectedDevice = defaultDevice.name
        } else if let firstDevice = devices.first {
            // Last resort: use the first device
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

        // Build HAL plugin chain with configured channels and metering
        let plugins: [[String: Any]] = [
            [
                "plugin_type": "hal_input",
                "parameters": ["channels": halInputChannels]
            ],
            [
                "plugin_type": "loudness_monitor",
                "parameters": ["channels": halInputChannels, "position": "input"]
            ],
            [
                "plugin_type": "loudness_monitor",
                "parameters": ["channels": halOutputChannels, "position": "output"]
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
            do {
                let data = try Data(contentsOf: url)
                let json = try JSONSerialization.jsonObject(with: data)

                var plugins: [[String: Any]]

                // Try parsing as simple array first (legacy format)
                if let simplePlugins = json as? [[String: Any]] {
                    plugins = simplePlugins
                }
                // Try parsing as complex format with channels (genelec.json format)
                else if let configDict = json as? [String: Any],
                        let channels = configDict["channels"] as? [String: Any] {
                    // Extract plugins from all channels and flatten
                    var allPlugins: [[String: Any]] = []

                    // Sort channel names for consistent ordering (L before R)
                    let sortedChannelNames = channels.keys.sorted()

                    for channelName in sortedChannelNames {
                        if let channelData = channels[channelName] as? [String: Any],
                           let channelPlugins = channelData["plugins"] as? [[String: Any]] {
                            // Add channel info to each plugin for context
                            for var plugin in channelPlugins {
                                plugin["_channel"] = channelName
                                allPlugins.append(plugin)
                            }
                        }
                    }

                    if allPlugins.isEmpty {
                        errorMessage = "No plugins found in channels configuration"
                        showingError = true
                        return
                    }

                    plugins = allPlugins

                    // Log what we found
                    if let version = configDict["version"] as? String {
                        print("Loading config version: \(version)")
                    }
                    print("Found \(channels.count) channel(s) with \(plugins.count) total plugins")
                }
                else {
                    errorMessage = "Invalid configuration format: expected array of plugins or object with 'channels'"
                    showingError = true
                    return
                }

                // Check if HAL plugins are already present
                let hasHalInput = plugins.contains { ($0["plugin_type"] as? String) == "hal_input" }
                let hasHalOutput = plugins.contains { ($0["plugin_type"] as? String) == "hal_output" }
                let hasLoudnessMonitor = plugins.contains { ($0["plugin_type"] as? String) == "loudness_monitor" }

                // Wrap with HAL plugins and metering if not present
                var finalPlugins: [[String: Any]] = []

                // HAL input
                if !hasHalInput {
                    finalPlugins.append([
                        "plugin_type": "hal_input",
                        "parameters": ["channels": halInputChannels]
                    ])
                }

                // Input metering (right after HAL input)
                if !hasLoudnessMonitor {
                    finalPlugins.append([
                        "plugin_type": "loudness_monitor",
                        "parameters": ["channels": halInputChannels, "position": "input"]
                    ])
                }

                // User's plugins
                finalPlugins.append(contentsOf: plugins)

                // Output metering (right before HAL output)
                if !hasLoudnessMonitor {
                    finalPlugins.append([
                        "plugin_type": "loudness_monitor",
                        "parameters": ["channels": halOutputChannels, "position": "output"]
                    ])
                }

                // HAL output
                if !hasHalOutput {
                    finalPlugins.append([
                        "plugin_type": "hal_output",
                        "parameters": ["channels": halOutputChannels]
                    ])
                }

                // Send plugins to daemon
                let command: [String: Any] = [
                    "command": "load_plugins",
                    "plugins": finalPlugins
                ]

                let response = client.sendCommand(command)
                if let resp = response, resp.success {
                    print("✅ Plugin configuration loaded from: \(url.path)")
                    print("   Total plugins: \(finalPlugins.count) (HAL wrapped: \(!hasHalInput || !hasHalOutput))")

                    // Update local state if HAL plugins found in original config
                    for plugin in plugins {
                        if let pluginType = plugin["plugin_type"] as? String,
                           let params = plugin["parameters"] as? [String: Any] {
                            if pluginType == "hal_input", let ch = params["channels"] as? Int {
                                halInputChannels = ch
                            }
                            if pluginType == "hal_output", let ch = params["channels"] as? Int {
                                halOutputChannels = ch
                            }
                        }
                    }
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
        panel.nameFieldStringValue = "autoeq_plugins.json"
        panel.message = "Save plugin configuration"

        if panel.runModal() == .OK, let url = panel.url {
            // Build current plugin configuration
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

            do {
                let data = try JSONSerialization.data(withJSONObject: plugins, options: .prettyPrinted)
                try data.write(to: url)
                print("✅ Plugin configuration saved to: \(url.path)")
            } catch {
                errorMessage = "Failed to save configuration: \(error.localizedDescription)"
                showingError = true
            }
        }
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
        if let status = client.getEncryptionStatus() {
            encryptionEnabled = status.enabled
            encryptionFingerprint = status.fingerprint
            encryptionError = nil
        } else {
            // Daemon might not be running
            encryptionFingerprint = ""
        }
    }
}

// MARK: - Plugin Host View (WebView for TypeScript UI)

struct PluginHostView: NSViewRepresentable {
    func makeNSView(context: Context) -> WKWebView {
        let webView = WKWebView()

        // Try to load from bundle resources first
        if let url = Bundle.main.url(forResource: "index", withExtension: "html", subdirectory: "ui") {
            webView.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        } else {
            // Show placeholder if UI not available
            let html = """
            <html>
            <head>
                <style>
                    body {
                        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
                        padding: 40px;
                        text-align: center;
                        color: #666;
                        background: #f5f5f7;
                    }
                    h2 { color: #333; margin-bottom: 20px; }
                    p { margin: 10px 0; line-height: 1.6; }
                    .icon { font-size: 48px; margin-bottom: 20px; }
                </style>
            </head>
            <body>
                <div class="icon">🎛️</div>
                <h2>Plugin Configuration</h2>
                <p>Advanced plugin configuration UI is not yet available.</p>
                <p>Use the channel settings above to configure HAL input/output,<br>
                   or load a plugin configuration JSON file.</p>
            </body>
            </html>
            """
            webView.loadHTMLString(html, baseURL: nil)
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

        print("AutoEQ menu bar app started")
    }

    func applicationWillTerminate(_ notification: Notification) {
        statusBarController?.stopMonitoring()
        statusBarController?.stopDaemon()
        print("AutoEQ menu bar app terminated")
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

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.run()
