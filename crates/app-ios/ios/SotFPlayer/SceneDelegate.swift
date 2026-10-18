import UIKit

/// Scene-based life-cycle entry point.
///
/// Apps built with the iOS 27 SDK must adopt the UIScene life cycle:
/// without a scene manifest and delegate, UIKit aborts the launch
/// before `AppDelegate.application(_:didFinishLaunchingWithOptions:)`
/// runs and the launch screen stays on display forever (black screen).
///
/// GPUI creates its `UIWindow` with the legacy `initWithFrame:`
/// initializer, which leaves it attached to no scene and therefore
/// invisible in a scene-based app (it does not appear in
/// `UIApplication.shared.windows`). So on connect we create a
/// scene-native window and host the GPUI Metal view inside its root
/// view controller through `gpui_ios_attach_to_view` — GPUI's
/// supported hosting entry point.
class SceneDelegate: UIResponder, UIWindowSceneDelegate {

    var window: UIWindow?
    private var gpuiWindowHandle: UnsafeMutableRawPointer?

    func scene(
        _ scene: UIScene,
        willConnectTo session: UISceneSession,
        options connectionOptions: UIScene.ConnectionOptions
    ) {
        guard let windowScene = scene as? UIWindowScene else {
            NSLog("[SceneDelegate] Connecting scene is not a UIWindowScene")
            return
        }
        guard let appDelegate = UIApplication.shared.delegate as? AppDelegate else {
            NSLog("[SceneDelegate] UIApplication delegate is not AppDelegate")
            return
        }

        // Create the GPUI window/view on first scene connection. This
        // also starts the CADisplayLink render tick.
        appDelegate.startGPUI()

        let hostWindow = UIWindow(windowScene: windowScene)
        let hostViewController = UIViewController()
        hostWindow.rootViewController = hostViewController
        window = hostWindow
        hostWindow.makeKeyAndVisible()

        gpuiWindowHandle = gpui_ios_attach_to_view(
            Unmanaged.passUnretained(hostViewController.view).toOpaque()
        )
        if gpuiWindowHandle == nil {
            NSLog("[SceneDelegate] gpui_ios_attach_to_view found no GPUI window")
        }
        gpui_ios_request_current_frame()
    }

    func sceneDidDisconnect(_ scene: UIScene) {
        if let gpuiWindowHandle {
            gpui_ios_detach_view(gpuiWindowHandle)
            self.gpuiWindowHandle = nil
        }
        window = nil
    }

    // MARK: - Lifecycle forwarding
    //
    // With the scene life cycle these replace the corresponding
    // UIApplicationDelegate callbacks, which UIKit no longer sends.

    func sceneWillEnterForeground(_ scene: UIScene) {
        gpui_ios_will_enter_foreground(nil)
    }

    func sceneDidBecomeActive(_ scene: UIScene) {
        gpui_ios_did_become_active(nil)
        gpui_ios_request_current_frame()
    }

    func sceneWillResignActive(_ scene: UIScene) {
        gpui_ios_will_resign_active(nil)
    }

    func sceneDidEnterBackground(_ scene: UIScene) {
        gpui_ios_did_enter_background(nil)
    }
}
