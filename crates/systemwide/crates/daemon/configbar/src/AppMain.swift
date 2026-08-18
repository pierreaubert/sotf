import Cocoa
import ConfigBarUI

// MARK: - App Delegate

final class AppDelegate: NSObject, NSApplicationDelegate {
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
