// EQViewController.swift
// UI for SOTF Parametric EQ
//
// Uses Rust Metal UI when available, with AppKit table fallback.

import AppKit
import CoreAudioKit
import AudioToolbox

// MARK: - EQ Filter Parameters

public struct EQFilterParams {
    public var frequency: Double  // Hz (20-20000)
    public var q: Double          // Quality factor (0.1-10.0)
    public var gainDb: Double     // Gain in decibels (-24 to +24)
    public var filterType: Int32  // 0=Peak, 1=LowShelf, 2=HighShelf, 3=Lowpass, 4=Highpass

    public static let frequencyRange = 20.0...20000.0
    public static let qRange = 0.1...10.0
    public static let gainRange = -24.0...24.0

    public static let filterTypeNames = ["Peak", "Low Shelf", "High Shelf", "Low Pass", "High Pass"]

    public init(frequency: Double, q: Double, gainDb: Double, filterType: Int32) {
        self.frequency = frequency
        self.q = q
        self.gainDb = gainDb
        self.filterType = filterType
    }

    public static func `default`() -> EQFilterParams {
        EQFilterParams(frequency: 1000.0, q: 1.0, gainDb: 0.0, filterType: 0)
    }

    public mutating func clamp() {
        frequency = min(max(frequency, Self.frequencyRange.lowerBound), Self.frequencyRange.upperBound)
        q = min(max(q, Self.qRange.lowerBound), Self.qRange.upperBound)
        gainDb = min(max(gainDb, Self.gainRange.lowerBound), Self.gainRange.upperBound)
        filterType = min(max(filterType, 0), Int32(Self.filterTypeNames.count - 1))
    }

    /// Convert to C struct
    func toCBand() -> CAUEQBand {
        return CAUEQBand(
            filter_type: filterType,
            frequency: Float(frequency),
            gain_db: Float(gainDb),
            q: Float(q),
            enabled: true
        )
    }

    /// Create from C struct
    static func fromCBand(_ band: CAUEQBand) -> EQFilterParams {
        return EQFilterParams(
            frequency: Double(band.frequency),
            q: Double(band.q),
            gainDb: Double(band.gain_db),
            filterType: band.filter_type
        )
    }
}

// MARK: - View Controller

public class EQViewController: AUViewController, AUAudioUnitFactory, NSTableViewDataSource, NSTableViewDelegate, NSTextFieldDelegate {

    // MARK: - Audio Unit

    /// The audio unit instance created by this view controller
    /// nonisolated(unsafe) to allow access from createAudioUnit which runs on non-main thread
    nonisolated(unsafe) private var audioUnit: EQAudioUnit?

    // Number of EQ bands
    private let bandCount = 5

    // EQ filter parameters
    private var filters: [EQFilterParams] = []

    // MARK: - Rust View

    /// Handle to Rust Metal view (NULL if unavailable)
    private var rustView: OpaquePointer?

    /// Whether we're using Rust UI (vs fallback AppKit)
    private var usingRustUI = false

    // MARK: - Fallback UI

    // Table view for parameter editing (fallback UI)
    private var tableView: NSTableView?
    private var scrollView: NSScrollView?

    // Callback for parameter changes
    public var onParametersChanged: (([EQFilterParams]) -> Void)?

    // Timer for polling Rust view for parameter changes
    private var pollTimer: Timer?

    // MARK: - AUAudioUnitFactory

    /// Creates and returns the Audio Unit instance.
    /// This is called by the AUv3 framework when the host requests the audio unit.
    /// Required by AUAudioUnitFactory protocol.
    /// Note: This is called from an XPC thread, not the main thread.
    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try EQAudioUnit(componentDescription: componentDescription, options: [])
        // Store the audio unit reference - this is safe because audioUnit is nonisolated(unsafe)
        self.audioUnit = unit
        // Schedule UI connection on main thread
        DispatchQueue.main.async { [weak self] in
            self?.connectToAudioUnit()
        }
        return unit
    }

    /// Connect UI controls to the audio unit's parameter tree
    private func connectToAudioUnit() {
        guard let audioUnit = audioUnit else { return }

        // Load initial parameter values from audio unit
        loadParametersFromAudioUnit()

        // Observe parameter changes from host/automation
        audioUnit.parameterTree?.implementorValueObserver = { [weak self] param, value in
            DispatchQueue.main.async {
                self?.parameterChangedFromHost(param: param, value: value)
            }
        }

        // Set up callback to update audio unit when UI changes
        onParametersChanged = { [weak self] filters in
            self?.syncFiltersToAudioUnit(filters)
        }
    }

    /// Load current parameter values from the audio unit
    private func loadParametersFromAudioUnit() {
        guard let audioUnit = audioUnit,
              let parameterTree = audioUnit.parameterTree else { return }

        for band in 0..<bandCount {
            let freqAddr = AUParameterAddress(band * 3)
            let gainAddr = AUParameterAddress(band * 3 + 1)
            let qAddr = AUParameterAddress(band * 3 + 2)

            if let freqParam = parameterTree.parameter(withAddress: freqAddr),
               let gainParam = parameterTree.parameter(withAddress: gainAddr),
               let qParam = parameterTree.parameter(withAddress: qAddr) {
                filters[band].frequency = Double(freqParam.value)
                filters[band].gainDb = Double(gainParam.value)
                filters[band].q = Double(qParam.value)
            }
        }

        // Update UI
        if usingRustUI {
            syncFiltersToRustView()
        } else {
            tableView?.reloadData()
        }
    }

    /// Handle parameter changes from host (automation, preset load, etc.)
    private func parameterChangedFromHost(param: AUParameter, value: AUValue) {
        let band = Int(param.address) / 3
        let paramType = Int(param.address) % 3

        guard band < filters.count else { return }

        switch paramType {
        case 0: // frequency
            filters[band].frequency = Double(value)
        case 1: // gain
            filters[band].gainDb = Double(value)
        case 2: // Q
            filters[band].q = Double(value)
        default:
            break
        }

        // Update UI
        if usingRustUI {
            syncFiltersToRustView()
        } else {
            tableView?.reloadData(forRowIndexes: IndexSet(integer: band), columnIndexes: IndexSet(integersIn: 0..<5))
        }
    }

    /// Sync filter values from UI to audio unit parameters
    private func syncFiltersToAudioUnit(_ filters: [EQFilterParams]) {
        guard let audioUnit = audioUnit,
              let parameterTree = audioUnit.parameterTree else { return }

        for (band, filter) in filters.enumerated() {
            let freqAddr = AUParameterAddress(band * 3)
            let gainAddr = AUParameterAddress(band * 3 + 1)
            let qAddr = AUParameterAddress(band * 3 + 2)

            if let freqParam = parameterTree.parameter(withAddress: freqAddr) {
                freqParam.value = AUValue(filter.frequency)
            }
            if let gainParam = parameterTree.parameter(withAddress: gainAddr) {
                gainParam.value = AUValue(filter.gainDb)
            }
            if let qParam = parameterTree.parameter(withAddress: qAddr) {
                qParam.value = AUValue(filter.q)
            }
        }
    }

    /// Sync filters to Rust view
    private func syncFiltersToRustView() {
        guard let rustView = rustView else { return }

        var cBands = filters.map { $0.toCBand() }
        au_plugin_view_set_bands(rustView, &cBands, cBands.count)
    }

    /// Poll Rust view for parameter changes
    @objc private func pollRustView() {
        guard let rustView = rustView else { return }

        var cBands = [CAUEQBand](repeating: CAUEQBand(), count: bandCount)
        let count = au_plugin_view_get_bands(rustView, &cBands, bandCount)

        guard count > 0 else { return }

        var changed = false
        for i in 0..<Int(count) {
            let newFilter = EQFilterParams.fromCBand(cBands[i])

            // Check if values changed significantly
            if abs(filters[i].frequency - newFilter.frequency) > 0.1 ||
               abs(filters[i].gainDb - newFilter.gainDb) > 0.01 ||
               abs(filters[i].q - newFilter.q) > 0.01 {
                filters[i] = newFilter
                changed = true
            }
        }

        if changed {
            onParametersChanged?(filters)
        }
    }

    // MARK: - Lifecycle

    public override func viewDidLoad() {
        super.viewDidLoad()

        // Initialize filters
        filters = (0..<bandCount).map { i in
            // Default frequencies spread across spectrum
            let defaultFreqs = [100.0, 300.0, 1000.0, 3000.0, 10000.0]
            var filter = EQFilterParams.default()
            filter.frequency = defaultFreqs[i]
            return filter
        }

        // Try to create Rust Metal view
        if trySetupRustUI() {
            usingRustUI = true
            NSLog("SOTF EQ: Using Rust Metal UI")

            // Start polling for UI changes
            pollTimer = Timer.scheduledTimer(
                timeInterval: 0.05, // 20 Hz
                target: self,
                selector: #selector(pollRustView),
                userInfo: nil,
                repeats: true
            )
        } else {
            usingRustUI = false
            NSLog("SOTF EQ: Falling back to AppKit table UI")
            setupParameterTableUI()
        }
    }

    public override func viewWillDisappear() {
        super.viewWillDisappear()
        pollTimer?.invalidate()
        pollTimer = nil
    }

    deinit {
        pollTimer?.invalidate()
        if let rustView = rustView {
            au_plugin_view_destroy(rustView)
        }
    }

    // MARK: - Rust UI Setup

    private func trySetupRustUI() -> Bool {
        let width = UInt32(view.bounds.width > 0 ? view.bounds.width : 600)
        let height = UInt32(view.bounds.height > 0 ? view.bounds.height : 400)

        guard let rustView = au_plugin_view_create(width, height) else {
            NSLog("SOTF EQ: Failed to create Rust view")
            return false
        }

        guard let nativePtr = au_plugin_view_get_native(rustView) else {
            NSLog("SOTF EQ: Failed to get native NSView from Rust")
            au_plugin_view_destroy(rustView)
            return false
        }

        // Cast to NSView
        let nativeView = Unmanaged<NSView>.fromOpaque(nativePtr).takeUnretainedValue()

        // Store the rust view handle
        self.rustView = rustView

        // Set up auto-resizing
        nativeView.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(nativeView)

        NSLayoutConstraint.activate([
            nativeView.topAnchor.constraint(equalTo: view.topAnchor),
            nativeView.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            nativeView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            nativeView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
        ])

        // Sync initial filters to Rust view
        syncFiltersToRustView()

        return true
    }

    // MARK: - Fallback UI Setup

    private func setupParameterTableUI() {
        // Dark background
        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor(calibratedRed: 0.12, green: 0.12, blue: 0.14, alpha: 1.0).cgColor

        // Main container
        let container = NSView()
        container.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(container)

        // Title
        let titleLabel = NSTextField(labelWithString: "SOTF Parametric EQ")
        titleLabel.font = NSFont.systemFont(ofSize: 20, weight: .bold)
        titleLabel.textColor = NSColor.white
        titleLabel.alignment = .center
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(titleLabel)

        // Subtitle
        let subtitleLabel = NSTextField(labelWithString: "\(bandCount)-Band Parametric Equalizer (Fallback UI)")
        subtitleLabel.font = NSFont.systemFont(ofSize: 12)
        subtitleLabel.textColor = NSColor(calibratedWhite: 0.6, alpha: 1.0)
        subtitleLabel.alignment = .center
        subtitleLabel.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(subtitleLabel)

        // Create table view
        let tableView = NSTableView()
        tableView.backgroundColor = NSColor.clear
        tableView.style = .plain
        tableView.gridStyleMask = [.solidHorizontalGridLineMask]
        tableView.gridColor = NSColor(calibratedWhite: 0.25, alpha: 1.0)
        tableView.rowHeight = 32
        tableView.intercellSpacing = NSSize(width: 10, height: 4)
        tableView.dataSource = self
        tableView.delegate = self

        // Add columns
        let columns: [(id: String, title: String, width: CGFloat)] = [
            ("band", "Band", 60),
            ("type", "Type", 90),
            ("frequency", "Frequency (Hz)", 120),
            ("q", "Q", 80),
            ("gain", "Gain (dB)", 100),
        ]

        for (id, title, width) in columns {
            let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier(id))
            column.title = title
            column.width = width
            column.minWidth = width * 0.7
            column.maxWidth = width * 1.5

            // Header styling
            let headerCell = NSTableHeaderCell()
            headerCell.title = title
            headerCell.font = NSFont.systemFont(ofSize: 11, weight: .semibold)
            headerCell.textColor = NSColor(calibratedWhite: 0.8, alpha: 1.0)
            column.headerCell = headerCell

            tableView.addTableColumn(column)
        }

        self.tableView = tableView

        // Scroll view for table
        let scrollView = NSScrollView()
        scrollView.documentView = tableView
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.autohidesScrollers = true
        scrollView.borderType = .noBorder
        scrollView.backgroundColor = NSColor.clear
        scrollView.drawsBackground = false
        scrollView.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(scrollView)
        self.scrollView = scrollView

        // Parameter limits info
        let limitsLabel = NSTextField(labelWithString: """
            Limits: Freq 20-20000 Hz | Q 0.1-10.0 | Gain -24 to +24 dB
            """)
        limitsLabel.font = NSFont.monospacedSystemFont(ofSize: 10, weight: .regular)
        limitsLabel.textColor = NSColor(calibratedWhite: 0.45, alpha: 1.0)
        limitsLabel.alignment = .center
        limitsLabel.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(limitsLabel)

        // Version info
        let versionLabel = NSTextField(labelWithString: "v0.5.3")
        versionLabel.font = NSFont.monospacedSystemFont(ofSize: 10, weight: .regular)
        versionLabel.textColor = NSColor(calibratedWhite: 0.35, alpha: 1.0)
        versionLabel.alignment = .center
        versionLabel.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(versionLabel)

        // Layout constraints
        NSLayoutConstraint.activate([
            // Container fills view with padding
            container.topAnchor.constraint(equalTo: view.topAnchor, constant: 16),
            container.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: -16),
            container.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 16),
            container.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -16),

            // Title at top
            titleLabel.topAnchor.constraint(equalTo: container.topAnchor),
            titleLabel.centerXAnchor.constraint(equalTo: container.centerXAnchor),

            // Subtitle below title
            subtitleLabel.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: 4),
            subtitleLabel.centerXAnchor.constraint(equalTo: container.centerXAnchor),

            // Table view
            scrollView.topAnchor.constraint(equalTo: subtitleLabel.bottomAnchor, constant: 16),
            scrollView.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            scrollView.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            scrollView.bottomAnchor.constraint(equalTo: limitsLabel.topAnchor, constant: -12),

            // Limits label
            limitsLabel.centerXAnchor.constraint(equalTo: container.centerXAnchor),
            limitsLabel.bottomAnchor.constraint(equalTo: versionLabel.topAnchor, constant: -8),

            // Version at bottom
            versionLabel.centerXAnchor.constraint(equalTo: container.centerXAnchor),
            versionLabel.bottomAnchor.constraint(equalTo: container.bottomAnchor),
        ])
    }

    // MARK: - NSTableViewDataSource

    public func numberOfRows(in tableView: NSTableView) -> Int {
        return filters.count
    }

    // MARK: - NSTableViewDelegate

    public func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard let columnId = tableColumn?.identifier.rawValue, row < filters.count else {
            return nil
        }

        let filter = filters[row]

        switch columnId {
        case "band":
            return makeLabelCell("Band \(row + 1)", color: bandColor(for: row))

        case "type":
            return makePopUpCell(
                items: EQFilterParams.filterTypeNames,
                selectedIndex: Int(filter.filterType),
                tag: row * 10 + 0
            )

        case "frequency":
            return makeTextFieldCell(
                value: filter.frequency,
                format: "%.1f",
                tag: row * 10 + 1
            )

        case "q":
            return makeTextFieldCell(
                value: filter.q,
                format: "%.2f",
                tag: row * 10 + 2
            )

        case "gain":
            return makeTextFieldCell(
                value: filter.gainDb,
                format: "%.1f",
                tag: row * 10 + 3
            )

        default:
            return nil
        }
    }

    public func tableView(_ tableView: NSTableView, rowViewForRow row: Int) -> NSTableRowView? {
        let rowView = NSTableRowView()
        rowView.backgroundColor = NSColor(calibratedRed: 0.16, green: 0.16, blue: 0.18, alpha: 1.0)
        return rowView
    }

    // MARK: - Cell Creation Helpers

    private func makeLabelCell(_ text: String, color: NSColor) -> NSView {
        let cell = NSTextField(labelWithString: text)
        cell.font = NSFont.systemFont(ofSize: 12, weight: .medium)
        cell.textColor = color
        cell.alignment = .center
        return cell
    }

    private func makeTextFieldCell(value: Double, format: String, tag: Int) -> NSView {
        let textField = NSTextField()
        textField.stringValue = String(format: format, value)
        textField.font = NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .regular)
        textField.textColor = NSColor.white
        textField.backgroundColor = NSColor(calibratedRed: 0.2, green: 0.2, blue: 0.22, alpha: 1.0)
        textField.isBordered = true
        textField.bezelStyle = .roundedBezel
        textField.alignment = .center
        textField.isEditable = true
        textField.isSelectable = true
        textField.tag = tag
        textField.delegate = self
        textField.target = self
        textField.action = #selector(textFieldDidEndEditing(_:))
        return textField
    }

    private func makePopUpCell(items: [String], selectedIndex: Int, tag: Int) -> NSView {
        let popUp = NSPopUpButton()
        popUp.addItems(withTitles: items)
        popUp.selectItem(at: selectedIndex)
        popUp.font = NSFont.systemFont(ofSize: 11)
        popUp.tag = tag
        popUp.target = self
        popUp.action = #selector(popUpDidChange(_:))
        return popUp
    }

    private func bandColor(for index: Int) -> NSColor {
        let colors: [NSColor] = [
            NSColor(calibratedRed: 0.95, green: 0.3, blue: 0.3, alpha: 1.0),   // Red
            NSColor(calibratedRed: 0.95, green: 0.6, blue: 0.2, alpha: 1.0),   // Orange
            NSColor(calibratedRed: 0.95, green: 0.85, blue: 0.2, alpha: 1.0),  // Yellow
            NSColor(calibratedRed: 0.3, green: 0.85, blue: 0.4, alpha: 1.0),   // Green
            NSColor(calibratedRed: 0.3, green: 0.6, blue: 0.95, alpha: 1.0),   // Blue
        ]
        return colors[index % colors.count]
    }

    // MARK: - Value Change Handlers

    @objc private func textFieldDidEndEditing(_ sender: NSTextField) {
        let tag = sender.tag
        let row = tag / 10
        let param = tag % 10

        guard row < filters.count else { return }

        let newValue = sender.doubleValue

        switch param {
        case 1: // frequency
            filters[row].frequency = newValue
        case 2: // q
            filters[row].q = newValue
        case 3: // gain
            filters[row].gainDb = newValue
        default:
            break
        }

        // Clamp values
        filters[row].clamp()

        // Update text field with clamped value
        switch param {
        case 1:
            sender.stringValue = String(format: "%.1f", filters[row].frequency)
        case 2:
            sender.stringValue = String(format: "%.2f", filters[row].q)
        case 3:
            sender.stringValue = String(format: "%.1f", filters[row].gainDb)
        default:
            break
        }

        notifyParametersChanged()
    }

    @objc private func popUpDidChange(_ sender: NSPopUpButton) {
        let tag = sender.tag
        let row = tag / 10

        guard row < filters.count else { return }

        filters[row].filterType = Int32(sender.indexOfSelectedItem)
        notifyParametersChanged()
    }

    private func notifyParametersChanged() {
        onParametersChanged?(filters)
    }

    // MARK: - NSTextFieldDelegate

    public func controlTextDidEndEditing(_ obj: Notification) {
        guard let textField = obj.object as? NSTextField else { return }
        textFieldDidEndEditing(textField)
    }

    // MARK: - Public Parameter API

    /// Update EQ filters from external source
    public func updateFilters(_ newFilters: [EQFilterParams]) {
        filters = newFilters.map { f in
            var filter = f
            filter.clamp()
            return filter
        }

        // Update UI
        if usingRustUI {
            syncFiltersToRustView()
        } else {
            tableView?.reloadData()
        }
    }

    /// Get current EQ filters
    public func getFilters() -> [EQFilterParams] {
        return filters
    }
}
