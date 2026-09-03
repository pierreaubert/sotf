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
            // Only resume when the system indicates playback should continue.
            // Without this gate every transient interruption (Siri, system
            // sound) would restart playback the user had deliberately paused —
            // same contract as iOS AudioManager.handleInterruption.
            if let optionsValue = info[AVAudioSessionInterruptionOptionKey] as? UInt {
                let options = AVAudioSession.InterruptionOptions(rawValue: optionsValue)
                if options.contains(.shouldResume) {
                    // Re-activate the audio session before resuming
                    try? AVAudioSession.sharedInstance().setActive(true)
                    sotf_tvos_audio_interrupted(false)
                }
            }
        @unknown default:
            break
        }
    }
}
