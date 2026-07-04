import UIKit
import AVFoundation

@main
class AppDelegate: UIResponder, UIApplicationDelegate {

    var window: UIWindow?
    private var displayLink: CADisplayLink?
    private let audioManager = TVAudioManager()

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        // Set up audio session for background playback
        audioManager.configureAudioSession()

        // Launch the GPUI app — creates the UIWindow, Metal view,
        // and opens the PlayerView.
        sotf_tvos_start()

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
        gpui_ios_will_terminate(nil)
    }
}

// MARK: - Swift functions callable from Rust

/// Return the path to the music directory inside the tvOS sandbox.
@_cdecl("sotf_tvos_get_music_directory")
func sotfTvosGetMusicDirectory() -> UnsafePointer<CChar>? {
    let documentsDir = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
    let musicDir = documentsDir.appendingPathComponent("Music")

    // Create the directory if it doesn't exist
    try? FileManager.default.createDirectory(at: musicDir, withIntermediateDirectories: true)

    let path = musicDir.path
    struct Static {
        static var buffer: [CChar] = []
    }
    Static.buffer = Array(path.utf8CString)
    return Static.buffer.withUnsafeBufferPointer { $0.baseAddress }
}
