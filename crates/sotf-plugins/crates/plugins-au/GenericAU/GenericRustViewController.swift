// GenericRustViewController.swift
// Shared base class for every SOTF AU view controller. Handles the
// boilerplate that every plugin VC needs (loading the GPUI host view,
// late-binding the audio unit) and — critically — sets a non-zero
// `loadView()` frame and a `preferredContentSize`, without which AUv3
// hosts (REAPER, Logic, etc.) render a blank window because GPUIAUView
// stays at .zero size.

import AppKit
import CoreAudioKit
import AudioToolbox

open class GenericRustViewController: AUViewController {

    // MARK: - Subclass overrides

    /// Plugin type tag (e.g. "EQ", "Compressor"). Must match the Rust
    /// plugin name used by GPUIAUView / `gpui_au_create_with_plugin`.
    open class var pluginType: String { fatalError("Subclass must override pluginType") }

    /// Initial / preferred plugin window size. Subclasses can override.
    open class var defaultSize: NSSize { NSSize(width: 800, height: 500) }

    // MARK: - State

    nonisolated(unsafe) public var audioUnit: GenericRustAudioUnit?
    nonisolated(unsafe) public var gpuiView: GPUIAUView?

    // MARK: - View lifecycle

    public override func loadView() {
        let size = Self.defaultSize
        self.view = NSView(frame: NSRect(origin: .zero, size: size))
    }

    public override func viewDidLoad() {
        super.viewDidLoad()
        preferredContentSize = Self.defaultSize

        let gpui = GPUIAUView(pluginType: Self.pluginType, audioUnit: audioUnit)
        gpui.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(gpui)
        NSLayoutConstraint.activate([
            gpui.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            gpui.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            gpui.topAnchor.constraint(equalTo: view.topAnchor),
            gpui.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        gpuiView = gpui
    }

    public override func viewDidLayout() {
        super.viewDidLayout()
        if let au = audioUnit, let gpui = gpuiView {
            gpui.connectAudioUnit(au)
        }
    }
}
