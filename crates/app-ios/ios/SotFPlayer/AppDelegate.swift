import UIKit
import AVFoundation

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
        let win = gpui_ios_get_window()
        if win != nil {
            gpui_ios_request_frame(win)
        }
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
        gpui_ios_will_terminate(nil)
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
