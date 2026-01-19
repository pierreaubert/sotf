#!/usr/bin/swift
//
// AutoEQ Menu Bar Application
//
// This app provides a menu bar interface for controlling the AutoEQ audio driver
// with notifications for status changes, config updates, and audio events.

import Cocoa
import UserNotifications

// MARK: - Configuration Manager

class ConfigManager {
    static let shared = ConfigManager()

    private let configPaths: [URL] = [
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".config/autoeq_driver/config.toml"),
        URL(fileURLWithPath: "/usr/local/etc/autoeq_driver/config.toml"),
        URL(fileURLWithPath: FileManager.default.currentDirectoryPath + "/test_config_phase3.toml")
    ]

    private var fileMonitor: DispatchSourceFileSystemObject?
    private var configPath: URL?

    var onConfigChange: (() -> Void)?

    func findConfig() -> URL? {
        for path in configPaths {
            if FileManager.default.fileExists(atPath: path.path) {
                configPath = path
                return path
            }
        }
        return nil
    }

    func startWatching() {
        guard let configPath = findConfig() else {
            print("No config file found")
            return
        }

        let fileDescriptor = open(configPath.path, O_EVTONLY)
        guard fileDescriptor >= 0 else {
            print("Failed to open config file for monitoring")
            return
        }

        fileMonitor = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: fileDescriptor,
            eventMask: .write,
            queue: DispatchQueue.global()
        )

        fileMonitor?.setEventHandler { [weak self] in
            DispatchQueue.main.async {
                NotificationManager.shared.showNotification(
                    title: "AutoEQ Configuration Updated",
                    body: "Configuration file has been modified"
                )
                self?.onConfigChange?()
            }
        }

        fileMonitor?.setCancelHandler {
            close(fileDescriptor)
        }

        fileMonitor?.resume()
        print("Started watching config file: \(configPath.path)")
    }

    func stopWatching() {
        fileMonitor?.cancel()
        fileMonitor = nil
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

    // Delegate method to show notifications even when app is active
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        completionHandler([.banner, .sound])
    }
}

// MARK: - Audio Monitor

class AudioMonitor {
    static let shared = AudioMonitor()

    private var isMonitoring = false
    private var timer: Timer?

    func startMonitoring() {
        guard !isMonitoring else { return }
        isMonitoring = true

        // Poll for audio activity every 5 seconds
        timer = Timer.scheduledTimer(withTimeInterval: 5.0, repeats: true) { [weak self] _ in
            self?.checkAudioStatus()
        }

        print("Started audio monitoring")
    }

    func stopMonitoring() {
        isMonitoring = false
        timer?.invalidate()
        timer = nil
        print("Stopped audio monitoring")
    }

    private func checkAudioStatus() {
        // This is a placeholder - in a real implementation, you would
        // communicate with the driver to get actual audio status
        // For now, we just check if the driver is loaded

        let task = Process()
        task.launchPath = "/usr/sbin/system_profiler"
        task.arguments = ["SPAudioDataType"]

        let pipe = Pipe()
        task.standardOutput = pipe

        do {
            try task.run()
            task.waitUntilExit()

            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            if let output = String(data: data, encoding: .utf8),
               output.contains("AutoEQ") {
                // Driver is loaded
            }
        } catch {
            print("Error checking audio status: \(error)")
        }
    }
}

// MARK: - Status Bar Item

class StatusBarController {
    private var statusItem: NSStatusItem
    private var menu: NSMenu
    private var isDriverEnabled = true

    init() {
        // Create status bar item
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)

        if let button = statusItem.button {
            // Use SF Symbol for audio
            if let image = NSImage(systemSymbolName: "waveform.circle.fill",
                                   accessibilityDescription: "AutoEQ") {
                image.isTemplate = true
                button.image = image
            } else {
                button.title = "♪"
            }
            button.toolTip = "AutoEQ Audio Driver"
        }

        // Create menu
        menu = NSMenu()
        setupMenu()
        statusItem.menu = menu

        // Start monitoring
        ConfigManager.shared.onConfigChange = { [weak self] in
            self?.updateStatus()
        }
        ConfigManager.shared.startWatching()
        AudioMonitor.shared.startMonitoring()

        // Show startup notification
        NotificationManager.shared.showNotification(
            title: "AutoEQ Started",
            body: "Menu bar app is running"
        )
    }

    private func setupMenu() {
        menu.removeAllItems()

        // Title
        let titleItem = NSMenuItem()
        titleItem.title = "AutoEQ Audio Driver"
        titleItem.isEnabled = false
        menu.addItem(titleItem)

        menu.addItem(NSMenuItem.separator())

        // Status
        let statusItem = NSMenuItem()
        statusItem.title = isDriverEnabled ? "Status: Enabled ✓" : "Status: Disabled"
        statusItem.isEnabled = false
        menu.addItem(statusItem)

        menu.addItem(NSMenuItem.separator())

        // Toggle Enable/Disable
        let toggleItem = NSMenuItem(
            title: isDriverEnabled ? "Disable Driver" : "Enable Driver",
            action: #selector(toggleDriver),
            keyEquivalent: "e"
        )
        toggleItem.target = self
        menu.addItem(toggleItem)

        // Reload Configuration
        let reloadItem = NSMenuItem(
            title: "Reload Configuration",
            action: #selector(reloadConfig),
            keyEquivalent: "r"
        )
        reloadItem.target = self
        menu.addItem(reloadItem)

        menu.addItem(NSMenuItem.separator())

        // Show Config
        let showConfigItem = NSMenuItem(
            title: "Show Configuration...",
            action: #selector(showConfig),
            keyEquivalent: "c"
        )
        showConfigItem.target = self
        menu.addItem(showConfigItem)

        // Edit Config
        let editConfigItem = NSMenuItem(
            title: "Edit Configuration...",
            action: #selector(editConfig),
            keyEquivalent: ""
        )
        editConfigItem.target = self
        menu.addItem(editConfigItem)

        menu.addItem(NSMenuItem.separator())

        // About
        let aboutItem = NSMenuItem(
            title: "About AutoEQ",
            action: #selector(showAbout),
            keyEquivalent: ""
        )
        aboutItem.target = self
        menu.addItem(aboutItem)

        menu.addItem(NSMenuItem.separator())

        // Quit
        let quitItem = NSMenuItem(
            title: "Quit AutoEQ",
            action: #selector(quit),
            keyEquivalent: "q"
        )
        quitItem.target = self
        menu.addItem(quitItem)
    }

    @objc private func toggleDriver() {
        isDriverEnabled.toggle()

        NotificationManager.shared.showNotification(
            title: "AutoEQ Driver",
            body: isDriverEnabled ? "Driver enabled" : "Driver disabled"
        )

        // Update menu
        setupMenu()
        updateStatus()
    }

    @objc private func reloadConfig() {
        NotificationManager.shared.showNotification(
            title: "AutoEQ Configuration",
            body: "Reloading configuration..."
        )

        // Restart Core Audio to reload the driver
        let task = Process()
        task.launchPath = "/usr/bin/sudo"
        task.arguments = ["launchctl", "kickstart", "-k", "system/com.apple.audio.coreaudiod"]

        do {
            try task.run()
            task.waitUntilExit()

            DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
                NotificationManager.shared.showNotification(
                    title: "AutoEQ Configuration",
                    body: "Configuration reloaded successfully"
                )
            }
        } catch {
            NotificationManager.shared.showNotification(
                title: "AutoEQ Error",
                body: "Failed to reload configuration: \(error.localizedDescription)"
            )
        }
    }

    @objc private func showConfig() {
        guard let configPath = ConfigManager.shared.findConfig() else {
            let alert = NSAlert()
            alert.messageText = "Configuration Not Found"
            alert.informativeText = "No configuration file found."
            alert.alertStyle = .warning
            alert.addButton(withTitle: "OK")
            alert.runModal()
            return
        }

        // Read config and show in alert
        if let config = try? String(contentsOf: configPath, encoding: .utf8) {
            let alert = NSAlert()
            alert.messageText = "AutoEQ Configuration"
            alert.informativeText = "Config file: \(configPath.path)\n\n\(config.prefix(500))..."
            alert.alertStyle = .informational
            alert.addButton(withTitle: "OK")
            alert.addButton(withTitle: "Open in Editor")

            let response = alert.runModal()
            if response == .alertSecondButtonReturn {
                editConfig()
            }
        }
    }

    @objc private func editConfig() {
        guard let configPath = ConfigManager.shared.findConfig() else {
            let alert = NSAlert()
            alert.messageText = "Configuration Not Found"
            alert.informativeText = "No configuration file found."
            alert.alertStyle = .warning
            alert.addButton(withTitle: "OK")
            alert.runModal()
            return
        }

        NSWorkspace.shared.open(configPath)
    }

    @objc private func showAbout() {
        let alert = NSAlert()
        alert.messageText = "AutoEQ Audio Driver"
        alert.informativeText = """
        Version 0.1.0

        A macOS Core Audio HAL driver with Audio Unit support.

        Features:
        • 16 input channels
        • 16 output channels
        • Audio Unit plugin chain
        • Real-time audio processing

        © 2025 Pierre. All rights reserved.
        """
        alert.alertStyle = .informational
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }

    @objc private func quit() {
        ConfigManager.shared.stopWatching()
        AudioMonitor.shared.stopMonitoring()

        NotificationManager.shared.showNotification(
            title: "AutoEQ",
            body: "Menu bar app quit"
        )

        NSApplication.shared.terminate(nil)
    }

    private func updateStatus() {
        // Update status bar icon or title based on driver state
        if let button = statusItem.button {
            button.appearsDisabled = !isDriverEnabled
        }
    }
}

// MARK: - App Delegate

class AppDelegate: NSObject, NSApplicationDelegate {
    var statusBarController: StatusBarController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Hide dock icon (menu bar only app)
        NSApp.setActivationPolicy(.accessory)

        // Create status bar controller
        statusBarController = StatusBarController()

        print("AutoEQ menu bar app started")
    }

    func applicationWillTerminate(_ notification: Notification) {
        print("AutoEQ menu bar app terminated")
    }
}

// MARK: - Main

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.run()
