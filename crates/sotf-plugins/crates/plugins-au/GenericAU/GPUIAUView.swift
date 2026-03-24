// GPUIAUView.swift
// NSView subclass that hosts GPUI rendering via Metal for Audio Unit plugin UIs.

import AppKit

public class GPUIAUView: NSView {

    private var gpuiContext: UnsafeMutableRawPointer?
    private var renderTimer: Timer?
    private let pluginType: String

    public init(pluginType: String) {
        self.pluginType = pluginType
        super.init(frame: .zero)
        wantsLayer = true
        // Red background so we can visually confirm the view is in the hierarchy
        layer?.backgroundColor = NSColor.red.cgColor
        NSLog("SOTF GPUIAUView: init pluginType=\(pluginType)")
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
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
        NSLog("SOTF GPUIAUView: layout, bounds=\(bounds), gpuiContext=\(gpuiContext != nil)")

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
            gpui_au_create(
                Unmanaged.passUnretained(self).toOpaque(),
                width,
                height,
                scale,
                typePtr
            )
        }

        if gpuiContext != nil {
            NSLog("SOTF GPUIAUView: context created OK, starting render timer")
            // Change background to dark once GPUI is initialized
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
