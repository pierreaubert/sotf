// GPUIAUView.swift
// NSView subclass that hosts GPUI rendering via Metal for Audio Unit plugin UIs.

import AppKit

public class GPUIAUView: NSView {

    private var gpuiContext: UnsafeMutableRawPointer?
    private var renderTimer: Timer?
    private let pluginType: String
    /// Atomic parameter cache for thread-safe UI rendering.
    private let paramCache: UnsafeMutableRawPointer?
    /// Reference to the AU for parameter writes through AUParameterTree.
    private weak var audioUnit: GenericRustAudioUnit?

    public init(pluginType: String, audioUnit: GenericRustAudioUnit? = nil) {
        self.pluginType = pluginType
        self.audioUnit = audioUnit

        // Create atomic param cache if we have an AU with parameters
        if let au = audioUnit, let tree = au.parameterTree {
            let count = tree.allParameters.count
            self.paramCache = au_param_cache_create(count)
            // Initialize cache with current parameter values
            for (i, param) in tree.allParameters.enumerated() {
                let denormalized = GPUIAUView.denormalizeParam(param)
                au_param_cache_write(self.paramCache, i, denormalized)
            }
        } else {
            self.paramCache = nil
        }

        super.init(frame: .zero)
        wantsLayer = true
        layer?.backgroundColor = NSColor.red.cgColor
        NSLog("SOTF GPUIAUView: init pluginType=\(pluginType), hasAU=\(audioUnit != nil)")

        // Observe AU parameter changes to update the cache
        if let au = audioUnit {
            setupParameterObservation(au)
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    // MARK: - Parameter Observation

    /// Wire AU parameter tree observer to push values into the atomic cache.
    private func setupParameterObservation(_ au: GenericRustAudioUnit) {
        guard let cache = paramCache, let tree = au.parameterTree else { return }

        // The implementorValueObserver is already set by GenericRustAudioUnit
        // to sync to Rust. We add a token-based observer for the UI cache.
        let allParams = tree.allParameters
        for (i, param) in allParams.enumerated() {
            let idx = i
            param.token(byAddingParameterObserver: { [weak self] _, value in
                guard let cache = self?.paramCache else { return }
                // Denormalize: AU parameters store AUValue (already denormalized in our setup)
                au_param_cache_write(cache, idx, Double(value))
            })
        }
    }

    /// Denormalize an AU parameter value to its real-world value.
    private static func denormalizeParam(_ param: AUParameter) -> Double {
        return Double(param.value)
    }

    // MARK: - GPUI Lifecycle

    public override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        NSLog("SOTF GPUIAUView: viewDidMoveToWindow, window=\(window != nil), bounds=\(bounds)")

        if window != nil && gpuiContext == nil {
            tryInitializeGPUI()
        } else if window == nil {
            teardownGPUI()
        }
    }

    public override func layout() {
        super.layout()
        if window != nil && gpuiContext == nil {
            tryInitializeGPUI()
        }
    }

    private func tryInitializeGPUI() {
        guard gpuiContext == nil else { return }

        let scale = Float(window?.backingScaleFactor ?? 2.0)
        let width = Float(bounds.width)
        let height = Float(bounds.height)

        guard width > 1 && height > 1 else {
            NSLog("SOTF GPUIAUView: skipping init, size too small: \(width)x\(height)")
            return
        }

        NSLog("SOTF GPUIAUView: creating GPUI context for \(pluginType) at \(width)x\(height) @\(scale)x")

        gpuiContext = pluginType.withCString { typePtr in
            if let cache = self.paramCache, let au = self.audioUnit {
                // Real plugin UI with thread-safe parameter bridge
                let userdata = Unmanaged.passUnretained(au).toOpaque()
                return gpui_au_create_with_plugin(
                    Unmanaged.passUnretained(self).toOpaque(),
                    width,
                    height,
                    scale,
                    typePtr,
                    cache,
                    gpuiSetParamCallback,
                    gpuiResetParamCallback,
                    userdata
                )
            } else {
                // Placeholder UI (no AU available)
                NSLog("SOTF GPUIAUView: no AU available, using placeholder UI")
                return gpui_au_create(
                    Unmanaged.passUnretained(self).toOpaque(),
                    width,
                    height,
                    scale,
                    typePtr
                )
            }
        }

        if gpuiContext != nil {
            NSLog("SOTF GPUIAUView: context created OK, starting render timer")
            layer?.backgroundColor = NSColor(calibratedRed: 0.1, green: 0.1, blue: 0.12, alpha: 1.0).cgColor
            renderTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in
                guard let self = self, let ctx = self.gpuiContext else { return }
                gpui_au_request_frame(ctx)
            }
        } else {
            NSLog("SOTF GPUIAUView: gpui_au_create returned NULL!")
        }
    }

    private func teardownGPUI() {
        renderTimer?.invalidate()
        renderTimer = nil

        if let ctx = gpuiContext {
            gpui_au_destroy(ctx)
            gpuiContext = nil
        }
    }

    deinit {
        teardownGPUI()
        if let cache = paramCache {
            au_param_cache_destroy(cache)
        }
    }

    // MARK: - Resize

    public override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        guard let ctx = gpuiContext else { return }
        let scale = Float(window?.backingScaleFactor ?? 2.0)
        gpui_au_resize(ctx, Float(newSize.width), Float(newSize.height), scale)
    }

    public override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        guard let ctx = gpuiContext else { return }
        let scale = Float(window?.backingScaleFactor ?? 2.0)
        gpui_au_resize(ctx, Float(bounds.width), Float(bounds.height), scale)
    }

    // MARK: - Mouse Events

    public override var acceptsFirstResponder: Bool { true }
    public override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    private func localPoint(for event: NSEvent) -> (Float, Float) {
        let point = convert(event.locationInWindow, from: nil)
        return (Float(point.x), Float(bounds.height - point.y))
    }

    public override func mouseDown(with event: NSEvent) {
        guard let ctx = gpuiContext else { return }
        let (x, y) = localPoint(for: event)
        gpui_au_mouse_down(ctx, x, y, 0, Int32(event.clickCount))
    }

    public override func mouseUp(with event: NSEvent) {
        guard let ctx = gpuiContext else { return }
        let (x, y) = localPoint(for: event)
        gpui_au_mouse_up(ctx, x, y, 0)
    }

    public override func mouseMoved(with event: NSEvent) {
        guard let ctx = gpuiContext else { return }
        let (x, y) = localPoint(for: event)
        gpui_au_mouse_moved(ctx, x, y)
    }

    public override func mouseDragged(with event: NSEvent) {
        guard let ctx = gpuiContext else { return }
        let (x, y) = localPoint(for: event)
        gpui_au_mouse_dragged(ctx, x, y, 0)
    }

    public override func rightMouseDown(with event: NSEvent) {
        guard let ctx = gpuiContext else { return }
        let (x, y) = localPoint(for: event)
        gpui_au_mouse_down(ctx, x, y, 1, Int32(event.clickCount))
    }

    public override func rightMouseUp(with event: NSEvent) {
        guard let ctx = gpuiContext else { return }
        let (x, y) = localPoint(for: event)
        gpui_au_mouse_up(ctx, x, y, 1)
    }

    public override func rightMouseDragged(with event: NSEvent) {
        guard let ctx = gpuiContext else { return }
        let (x, y) = localPoint(for: event)
        gpui_au_mouse_dragged(ctx, x, y, 1)
    }

    public override func scrollWheel(with event: NSEvent) {
        guard let ctx = gpuiContext else { return }
        let (x, y) = localPoint(for: event)
        gpui_au_scroll_wheel(ctx, x, y, Float(event.scrollingDeltaX), Float(event.scrollingDeltaY))
    }

    public override func updateTrackingAreas() {
        super.updateTrackingAreas()
        for area in trackingAreas {
            removeTrackingArea(area)
        }
        addTrackingArea(NSTrackingArea(
            rect: bounds,
            options: [.mouseMoved, .activeInKeyWindow, .inVisibleRect],
            owner: self,
            userInfo: nil
        ))
    }
}

// MARK: - C Callbacks for GPUI → AUParameterTree

/// Called by GPUI when the user changes a parameter via the UI.
/// Routes through AUParameterTree for thread-safe dispatch to the audio plugin.
private func gpuiSetParamCallback(userdata: UnsafeMutableRawPointer?, paramIndex: Int, value: Double) {
    guard let ud = userdata else { return }
    let au = Unmanaged<GenericRustAudioUnit>.fromOpaque(ud).takeUnretainedValue()
    guard let tree = au.parameterTree else { return }
    let allParams = tree.allParameters
    guard paramIndex < allParams.count else { return }
    let param = allParams[paramIndex]
    // Set via AUParameterTree — this triggers implementorValueObserver → plugin_set_parameter
    param.value = AUValue(value)
}

/// Called by GPUI when the user resets a parameter to its default.
private func gpuiResetParamCallback(userdata: UnsafeMutableRawPointer?, paramIndex: Int) {
    guard let ud = userdata else { return }
    let au = Unmanaged<GenericRustAudioUnit>.fromOpaque(ud).takeUnretainedValue()
    guard let tree = au.parameterTree else { return }
    let allParams = tree.allParameters
    guard paramIndex < allParams.count else { return }
    let param = allParams[paramIndex]
    // Reset to AU default value
    let info = plugin_get_parameter_info(au.pluginHandle, paramIndex)
    if let info = info {
        param.value = AUValue(info.pointee.default_value)
    }
}
