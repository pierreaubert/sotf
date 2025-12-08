// EQViewController.swift
// UI for SOTF Parametric EQ

import AppKit
import CoreAudioKit
import AudioToolbox

// MARK: - View Controller

public class EQViewController: AUViewController {
    public override func viewDidLoad() {
        super.viewDidLoad()
        setupUI()
    }

    private func setupUI() {
        // Note: In modern AUViewController (CoreAudioKit), the audio unit
        // is not directly accessible via a simple property.
        // For now, create a simple placeholder view.

        let label = NSTextField(labelWithString: """
            SOTF Parametric EQ

            Audio Unit is loaded and processing audio.
            UI controls will be available in a future update.
            """)
        label.alignment = .center
        label.isBezeled = false
        label.drawsBackground = false
        label.isEditable = false
        label.isSelectable = false
        label.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(label)

        NSLayoutConstraint.activate([
            label.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            label.centerYAnchor.constraint(equalTo: view.centerYAnchor),
        ])
    }
}
