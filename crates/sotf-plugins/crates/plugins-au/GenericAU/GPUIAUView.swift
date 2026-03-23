// GPUIAUView.swift
// NSView subclass that hosts GPUI rendering via Metal for Audio Unit plugin UIs.
//
// This view:
// 1. Creates a GPUI rendering context via FFI (gpui_au_create)
// 2. Drives frame rendering via NSTimer
// 3. Forwards mouse/keyboard events to GPUI via FFI
// 4. Handles resize notifications

import AppKit

public class GPUIAUView: NSView {

    private var gpuiContext: UnsafeMutableRawPointer?
    private var renderTimer: Timer?
    private let pluginType: String

    public init(pluginType: String) {
        self.pluginType = pluginType
        super.init(frame: .zero)
        wantsLayer = true
        layer?.backgroundColor = NSColor(calibratedRed: 0.1, green: 0.1, blue: 0.12, alpha: 1.0).cgColor
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    // MARK: - GPUI Lifecycle

    public override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()

        if window != nil && gpuiContext == nil {
            initializeGPUI()
        } else if window == nil {
            teardownGPUI()
        }
    }

    private func initializeGPUI() {
        let scale = Float(window?.backingScaleFactor ?? 2.0)
        let width = Float(bounds.width)
        let height = Float(bounds.height)

        guard width > 0 && height > 0 else {
            // View not yet sized — will be called again after layout
            return
        }

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
            // 60 FPS render timer on the main run loop
            renderTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in
                guard let self = self, let ctx = self.gpuiContext else { return }
                gpui_au_request_frame(ctx)
            }
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

    private func buttonIndex(for event: NSEvent) -> Int32 {
        switch event.type {
        case .rightMouseDown, .rightMouseUp, .rightMouseDragged:
            return 1
        case .otherMouseDown, .otherMouseUp, .otherMouseDragged:
            return 2
        default:
            return 0
        }
    }

    private func localPoint(for event: NSEvent) -> (Float, Float) {
        let point = convert(event.locationInWindow, from: nil)
        // GPUI uses top-left origin; NSView uses bottom-left
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

    // MARK: - Tracking Area (for mouseMoved events)

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
