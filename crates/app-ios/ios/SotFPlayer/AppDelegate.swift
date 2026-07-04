import UIKit
import AVFoundation
import MediaPlayer

@main
class AppDelegate: UIResponder, UIApplicationDelegate {

    var window: UIWindow?
    private var displayLink: CADisplayLink?
    private let audioManager = AudioManager()

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        // Set up audio session for background playback
        audioManager.configureAudioSession()
        configurePlatformObservers(application)

        // Launch the GPUI app — creates the UIWindow, Metal view,
        // and opens the PlayerView. Returns immediately because on iOS
        // the run loop is managed by UIApplicationMain.
        sotf_ios_start()

        // Set up a CADisplayLink so GPUI gets a render tick every frame.
        displayLink = CADisplayLink(target: self, selector: #selector(renderFrame))
        displayLink?.add(to: .main, forMode: .common)

        return true
    }

    @objc private func renderFrame() {
        gpui_ios_request_current_frame()
    }

    // MARK: - Lifecycle forwarding

    func applicationWillEnterForeground(_ application: UIApplication) {
        gpui_ios_will_enter_foreground(nil)
    }

    func applicationDidBecomeActive(_ application: UIApplication) {
        gpui_ios_did_become_active(nil)
    }

    func applicationWillResignActive(_ application: UIApplication) {
        gpui_ios_will_resign_active(nil)
    }

    func applicationDidEnterBackground(_ application: UIApplication) {
        gpui_ios_did_enter_background(nil)
    }

    func applicationWillTerminate(_ application: UIApplication) {
        NotificationCenter.default.removeObserver(self)
        gpui_ios_will_terminate(nil)
    }

    func applicationDidReceiveMemoryWarning(_ application: UIApplication) {
        NSLog("[AppDelegate] Memory warning received")
        sotf_ios_memory_warning()
    }

    private func configurePlatformObservers(_ application: UIApplication) {
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleDynamicTypeChanged),
            name: UIContentSizeCategory.didChangeNotification,
            object: nil
        )

        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleLowPowerModeChanged),
            name: .NSProcessInfoPowerStateDidChange,
            object: nil
        )

        sotf_ios_dynamic_type_scale_changed(Self.dynamicTypeScale(for: application.preferredContentSizeCategory))
        sotf_ios_low_power_mode_changed(ProcessInfo.processInfo.isLowPowerModeEnabled)
    }

    @objc private func handleDynamicTypeChanged(notification: Notification) {
        let category = UIApplication.shared.preferredContentSizeCategory
        let scale = Self.dynamicTypeScale(for: category)
        NSLog("[AppDelegate] Dynamic Type changed: \(category.rawValue), scale=\(scale)")
        sotf_ios_dynamic_type_scale_changed(scale)
    }

    @objc private func handleLowPowerModeChanged(notification: Notification) {
        let enabled = ProcessInfo.processInfo.isLowPowerModeEnabled
        NSLog("[AppDelegate] Low Power Mode changed: \(enabled)")
        sotf_ios_low_power_mode_changed(enabled)
    }

    private static func dynamicTypeScale(for category: UIContentSizeCategory) -> Double {
        switch category {
        case .extraSmall: return 0.85
        case .small: return 0.92
        case .medium: return 0.96
        case .large: return 1.0
        case .extraLarge: return 1.08
        case .extraExtraLarge: return 1.16
        case .extraExtraExtraLarge: return 1.24
        case .accessibilityMedium: return 1.34
        case .accessibilityLarge: return 1.48
        case .accessibilityExtraLarge: return 1.62
        case .accessibilityExtraExtraLarge: return 1.78
        case .accessibilityExtraExtraExtraLarge: return 1.95
        default: return 1.0
        }
    }
}

// MARK: - Swift functions callable from Rust

/// Present the iOS document picker for importing music files.
/// Called from Rust when the user taps "Import Music".
@_cdecl("sotf_ios_show_document_picker")
func sotfIosShowDocumentPicker() {
    DispatchQueue.main.async {
        DocumentPicker.shared.presentPicker()
    }
}

/// Present the native AirPlay/Bluetooth route picker.
/// Called from Rust when the user taps "AirPlay" in audio device settings.
@_cdecl("sotf_ios_show_route_picker")
func sotfIosShowRoutePicker() {
    DispatchQueue.main.async {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let rootVC = windowScene.windows.first?.rootViewController else {
            NSLog("[AppDelegate] No root view controller to present AirPlay route picker")
            return
        }

        struct RoutePickerHost {
            static var view: MPVolumeView?
        }

        let routePicker = MPVolumeView(frame: CGRect(x: -1000, y: -1000, width: 44, height: 44))
        routePicker.showsVolumeSlider = false
        RoutePickerHost.view = routePicker
        rootVC.view.addSubview(routePicker)

        for subview in routePicker.subviews where subview is UIButton {
            (subview as? UIButton)?.sendActions(for: .touchUpInside)
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                RoutePickerHost.view?.removeFromSuperview()
                RoutePickerHost.view = nil
            }
            return
        }

        RoutePickerHost.view?.removeFromSuperview()
        RoutePickerHost.view = nil
        NSLog("[AppDelegate] AirPlay route picker button not found")
    }
}

/// Return the path to the music directory inside the iOS sandbox.
/// The returned pointer is valid until the next call (static storage).
@_cdecl("sotf_ios_get_music_directory")
func sotfIosGetMusicDirectory() -> UnsafePointer<CChar>? {
    let path = DocumentPicker.musicDirectory.path
    // Store in a static to keep the C string alive
    struct Static {
        static var buffer: [CChar] = []
    }
    Static.buffer = Array(path.utf8CString)
    return Static.buffer.withUnsafeBufferPointer { $0.baseAddress }
}
