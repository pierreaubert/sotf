import AVFoundation
import MediaPlayer
import UIKit

/// Manages the iOS audio session lifecycle, interruptions, route changes,
/// and Now Playing / remote command center integration.
class AudioManager: NSObject {

    func configureAudioSession() {
        let session = AVAudioSession.sharedInstance()

        do {
            try session.setCategory(.playback, mode: .default, options: [])
            try session.setActive(true)
            NSLog("[AudioManager] Audio session configured: category=playback")
        } catch {
            NSLog("[AudioManager] Failed to configure audio session: \(error)")
        }

        // Interruptions (phone calls, alarms, Siri)
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleInterruption),
            name: AVAudioSession.interruptionNotification,
            object: session
        )

        // Route changes (headphone plug/unplug, Bluetooth connect/disconnect)
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleRouteChange),
            name: AVAudioSession.routeChangeNotification,
            object: session
        )

        // Set up remote command center (lock screen / Control Center controls)
        configureRemoteCommands()
    }

    // MARK: - Interruption Handling

    @objc private func handleInterruption(notification: Notification) {
        guard let userInfo = notification.userInfo,
              let typeValue = userInfo[AVAudioSessionInterruptionTypeKey] as? UInt,
              let type = AVAudioSession.InterruptionType(rawValue: typeValue) else {
            return
        }

        switch type {
        case .began:
            NSLog("[AudioManager] Audio interruption began — pausing")
            sotf_ios_audio_interrupted(true)

        case .ended:
            NSLog("[AudioManager] Audio interruption ended")
            if let optionsValue = userInfo[AVAudioSessionInterruptionOptionKey] as? UInt {
                let options = AVAudioSession.InterruptionOptions(rawValue: optionsValue)
                if options.contains(.shouldResume) {
                    NSLog("[AudioManager] Resuming after interruption")
                    // Reactivate the audio session before resuming
                    try? AVAudioSession.sharedInstance().setActive(true)
                    sotf_ios_audio_interrupted(false)
                }
            }

        @unknown default:
            break
        }
    }

    // MARK: - Route Change Handling

    @objc private func handleRouteChange(notification: Notification) {
        guard let userInfo = notification.userInfo,
              let reasonValue = userInfo[AVAudioSessionRouteChangeReasonKey] as? UInt,
              let reason = AVAudioSession.RouteChangeReason(rawValue: reasonValue) else {
            return
        }

        switch reason {
        case .oldDeviceUnavailable:
            if shouldPauseForUnavailableRoute(notification: notification) {
                NSLog("[AudioManager] Route changed: wired headphones unavailable — pausing")
                sotf_ios_audio_route_changed()
            } else {
                NSLog("[AudioManager] Route changed: device unavailable — continuing playback")
            }

        case .newDeviceAvailable:
            NSLog("[AudioManager] Route changed: new device available")

        default:
            NSLog("[AudioManager] Route changed: reason=\(reason.rawValue)")
        }
    }

    private func shouldPauseForUnavailableRoute(notification: Notification) -> Bool {
        guard let previousRoute = notification.userInfo?[AVAudioSessionRouteChangePreviousRouteKey] as? AVAudioSessionRouteDescription else {
            return false
        }

        return previousRoute.outputs.contains { output in
            output.portType == .headphones || output.portType == .headsetMic
        }
    }

    // MARK: - Remote Command Center (Lock Screen / Control Center)

    private func configureRemoteCommands() {
        let commandCenter = MPRemoteCommandCenter.shared()

        commandCenter.playCommand.isEnabled = true
        commandCenter.playCommand.addTarget { _ in
            sotf_ios_remote_play()
            return .success
        }

        commandCenter.pauseCommand.isEnabled = true
        commandCenter.pauseCommand.addTarget { _ in
            sotf_ios_remote_pause()
            return .success
        }

        commandCenter.togglePlayPauseCommand.isEnabled = true
        commandCenter.togglePlayPauseCommand.addTarget { _ in
            // Toggle: the Rust side checks current state
            sotf_ios_remote_toggle_play_pause()
            return .success
        }

        commandCenter.nextTrackCommand.isEnabled = true
        commandCenter.nextTrackCommand.addTarget { _ in
            sotf_ios_remote_next_track()
            return .success
        }

        commandCenter.previousTrackCommand.isEnabled = true
        commandCenter.previousTrackCommand.addTarget { _ in
            sotf_ios_remote_prev_track()
            return .success
        }

        commandCenter.changePlaybackPositionCommand.isEnabled = false
        commandCenter.changePlaybackPositionCommand.addTarget { event in
            guard let positionEvent = event as? MPChangePlaybackPositionCommandEvent else {
                return .commandFailed
            }
            sotf_ios_remote_seek(positionEvent.positionTime)
            return .success
        }

        NSLog("[AudioManager] Remote commands configured")
    }

    deinit {
        NotificationCenter.default.removeObserver(self)
    }
}

// MARK: - Now Playing Info (called from Rust via @_cdecl)

/// Update full Now Playing info (track change)
@_cdecl("sotf_ios_update_now_playing")
func sotfIosUpdateNowPlaying(
    title: UnsafePointer<CChar>?,
    artist: UnsafePointer<CChar>?,
    album: UnsafePointer<CChar>?,
    duration: Double,
    position: Double,
    isPlaying: Bool
) {
    var info = [String: Any]()
    let isSeekable = duration > 0

    if let title = title {
        info[MPMediaItemPropertyTitle] = String(cString: title)
    }
    if let artist = artist {
        info[MPMediaItemPropertyArtist] = String(cString: artist)
    }
    if let album = album {
        info[MPMediaItemPropertyAlbumTitle] = String(cString: album)
    }
    if isSeekable {
        info[MPMediaItemPropertyPlaybackDuration] = duration
    }
    info[MPNowPlayingInfoPropertyElapsedPlaybackTime] = position
    info[MPNowPlayingInfoPropertyPlaybackRate] = isPlaying ? 1.0 : 0.0

    MPNowPlayingInfoCenter.default().nowPlayingInfo = info
    MPRemoteCommandCenter.shared().changePlaybackPositionCommand.isEnabled = isSeekable
}

/// Update position only (periodic update, no track metadata change)
@_cdecl("sotf_ios_update_now_playing_position")
func sotfIosUpdateNowPlayingPosition(position: Double, isPlaying: Bool) {
    if var info = MPNowPlayingInfoCenter.default().nowPlayingInfo {
        info[MPNowPlayingInfoPropertyElapsedPlaybackTime] = position
        info[MPNowPlayingInfoPropertyPlaybackRate] = isPlaying ? 1.0 : 0.0
        MPNowPlayingInfoCenter.default().nowPlayingInfo = info
    }
}
