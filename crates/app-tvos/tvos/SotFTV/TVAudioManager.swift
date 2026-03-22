import AVFoundation
import UIKit

/// Manages the AVAudioSession for tvOS background audio playback.
///
/// tvOS differences from iOS:
/// - No MPRemoteCommandCenter (Siri Remote buttons handled via UIResponder)
/// - No MPNowPlayingInfoCenter (no lock screen / Control Center metadata)
/// - Audio session uses .duckOthers to play alongside system sounds
class TVAudioManager: NSObject {

    /// Configure the AVAudioSession for media playback.
    func configureAudioSession() {
        let session = AVAudioSession.sharedInstance()
        do {
            try session.setCategory(.playback, mode: .default, options: [.duckOthers])
            try session.setActive(true)
            print("[tvOS Audio] Session configured for playback")
        } catch {
            print("[tvOS Audio] Failed to configure session: \(error)")
        }

        // Listen for audio interruptions (e.g. Siri activation)
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleInterruption),
            name: AVAudioSession.interruptionNotification,
            object: session
        )
    }

    @objc private func handleInterruption(notification: Notification) {
        guard let info = notification.userInfo,
              let typeValue = info[AVAudioSessionInterruptionTypeKey] as? UInt,
              let type = AVAudioSession.InterruptionType(rawValue: typeValue) else {
            return
        }

        switch type {
        case .began:
            sotf_tvos_audio_interrupted(true)
        case .ended:
            sotf_tvos_audio_interrupted(false)
            // Re-activate the audio session
            try? AVAudioSession.sharedInstance().setActive(true)
        @unknown default:
            break
        }
    }
}
