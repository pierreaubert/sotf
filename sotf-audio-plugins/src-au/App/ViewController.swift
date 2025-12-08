// ViewController.swift
// SOTF Audio Units - Minimal container app view

import Cocoa

class ViewController: NSViewController {
    override func viewDidLoad() {
        super.viewDidLoad()

        // Create a simple label explaining this is just a container
        let label = NSTextField(labelWithString: """
            SOTF Audio Units

            This app contains Audio Unit plugins.

            To use them:
            1. The plugins are installed automatically
            2. Open your DAW (Logic Pro, GarageBand, etc.)
            3. Look for "SOTF: Parametric EQ" in the effects

            You don't need to run this app directly.
            """)

        label.alignment = .center
        label.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(label)

        NSLayoutConstraint.activate([
            label.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            label.centerYAnchor.constraint(equalTo: view.centerYAnchor),
            label.widthAnchor.constraint(lessThanOrEqualTo: view.widthAnchor, constant: -40)
        ])
    }
}
