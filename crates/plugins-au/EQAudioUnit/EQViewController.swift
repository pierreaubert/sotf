// EQViewController.swift
// View Controller for SOTF Audio Units
//
// Phase 1: Uses the host's auto-generated parameter UI.
// Phase 2 will add custom Metal-based visualizations.

import AppKit
import CoreAudioKit
import AudioToolbox

/// Generic view controller for SOTF Audio Units.
/// Creates the audio unit and lets the host generate the parameter UI.
public class EQViewController: AUViewController, AUAudioUnitFactory {

    nonisolated(unsafe) private var audioUnit: EQAudioUnit?

    /// Creates and returns the Audio Unit instance.
    /// Called by the AUv3 framework from an XPC thread.
    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try EQAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }

    public override func viewDidLoad() {
        super.viewDidLoad()

        // Minimal placeholder view
        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor(calibratedRed: 0.12, green: 0.12, blue: 0.14, alpha: 1.0).cgColor

        let label = NSTextField(labelWithString: "SOTF: Parametric EQ\nUse host controls for parameters")
        label.font = NSFont.systemFont(ofSize: 14, weight: .medium)
        label.textColor = NSColor(calibratedWhite: 0.7, alpha: 1.0)
        label.alignment = .center
        label.maximumNumberOfLines = 0
        label.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(label)

        NSLayoutConstraint.activate([
            label.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            label.centerYAnchor.constraint(equalTo: view.centerYAnchor),
        ])
    }
}
