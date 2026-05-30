// GenericRustAudioUnit.swift
// Base class for all SOTF Audio Units that delegate to Rust plugins.
//
// Subclasses only need to override pluginType and pluginSubtype.

import AVFoundation
import AudioToolbox
import CoreAudioKit
import Foundation
import UniformTypeIdentifiers

// MARK: - Render State (shared between main thread and render thread via pointer)

/// Mutable render state shared with the real-time render block via UnsafeMutablePointer.
/// The render block captures a pointer to this; allocateRenderResources updates the fields.
private struct RenderState {
    var handle: OpaquePointer?
    var channels: Int
    var scratchIn: UnsafeMutablePointer<Float>?
    var scratchOut: UnsafeMutablePointer<Float>?
    var scratchCapacity: Int
    var midiIn: UnsafeMutablePointer<PluginMidiEvent>?
    var midiOut: UnsafeMutablePointer<PluginMidiEvent>?
    var noteExpressionOut: UnsafeMutablePointer<PluginNoteExpressionEvent>?
    var eventCapacity: Int
}

/// Base AUAudioUnit that delegates all processing to a Rust plugin via FFI.
///
/// Subclasses must override:
/// - `pluginType` (e.g., "Compressor")
/// - `pluginSubtype` (e.g., "SOCP", 4-char code)
/// - `pluginName` (e.g., "SOTF: Compressor")
open class GenericRustAudioUnit: AUAudioUnit {

    // MARK: - Subclass Configuration (override these)

    /// Rust plugin type name passed to plugin_create()
    open class var pluginType: String { fatalError("Subclass must override pluginType") }

    /// AU subtype (4-char code)
    open class var pluginSubtype: String { fatalError("Subclass must override pluginSubtype") }

    /// Display name shown in DAW
    open class var pluginName: String { "SOTF Plugin" }

    // MARK: - Properties

    private var inputBus: AUAudioUnitBus
    private var outputBus: AUAudioUnitBus
    private var _inputBusArray: AUAudioUnitBusArray!
    private var _outputBusArray: AUAudioUnitBusArray!
    private var _parameterTree: AUParameterTree?
    private var auParameters: [AUParameter] = []
    private var _maxFramesToRender: UInt32 = 4096

    /// Current Rust plugin configuration — used to detect when re-creation is needed
    private var rustSampleRate: UInt32 = 0

    /// Heap-allocated render state shared with the render block via pointer.
    /// The render block captures `renderStatePtr` once; allocateRenderResources
    /// updates the pointed-to struct so the render thread always sees current values.
    private let renderStatePtr: UnsafeMutablePointer<RenderState>

    /// Public access to the Rust PluginHandle for GPUI UI rendering.
    /// Returns the raw pointer as `UnsafeMutableRawPointer?` for passing to
    /// `gpui_au_create_with_plugin()`. Valid as long as this AU instance is alive.
    public var pluginHandle: UnsafeMutableRawPointer? {
        guard let handle = renderStatePtr.pointee.handle else { return nil }
        return UnsafeMutableRawPointer(handle)
    }

    // MARK: - Initialization

    public override init(componentDescription: AudioComponentDescription,
                        options: AudioComponentInstantiationOptions = []) throws {
        // Allocate render state on the heap
        renderStatePtr = .allocate(capacity: 1)
        renderStatePtr.initialize(to: RenderState(
            handle: nil,
            channels: 0,
            scratchIn: nil,
            scratchOut: nil,
            scratchCapacity: 0,
            midiIn: nil,
            midiOut: nil,
            noteExpressionOut: nil,
            eventCapacity: 0
        ))

        // Start with a default stereo format; the host will set the actual format before rendering
        guard let defaultFormat = AVAudioFormat(standardFormatWithSampleRate: 48000, channels: 2) else {
            renderStatePtr.deallocate()
            throw NSError(domain: NSOSStatusErrorDomain, code: Int(kAudioUnitErr_FormatNotSupported))
        }

        inputBus = try AUAudioUnitBus(format: defaultFormat)
        outputBus = try AUAudioUnitBus(format: defaultFormat)

        // Allow any channel count up to 64
        inputBus.maximumChannelCount = 64
        outputBus.maximumChannelCount = 64

        try super.init(componentDescription: componentDescription, options: options)

        _inputBusArray = AUAudioUnitBusArray(audioUnit: self, busType: .input, busses: [inputBus])
        _outputBusArray = AUAudioUnitBusArray(audioUnit: self, busType: .output, busses: [outputBus])

        // Create initial Rust plugin with default format
        let channels = Int(defaultFormat.channelCount)
        let sampleRate = UInt32(defaultFormat.sampleRate)
        createRustPlugin(channels: channels, sampleRate: sampleRate)
        buildParameterTree()
    }

    deinit {
        let state = renderStatePtr.pointee
        if let handle = state.handle {
            plugin_destroy(handle)
        }
        state.scratchIn?.deallocate()
        state.scratchOut?.deallocate()
        state.midiIn?.deallocate()
        state.midiOut?.deallocate()
        state.noteExpressionOut?.deallocate()
        renderStatePtr.deallocate()
    }

    // MARK: - Rust Plugin Lifecycle

    private func createRustPlugin(channels: Int, sampleRate: UInt32) {
        // Destroy old handle if present
        if let handle = renderStatePtr.pointee.handle {
            plugin_destroy(handle)
            renderStatePtr.pointee.handle = nil
        }

        let pluginType = type(of: self).pluginType

        let handle = pluginType.withCString { typePtr in
            "{}".withCString { configPtr in
                plugin_create(typePtr, configPtr, sampleRate, channels, channels)
            }
        }

        if let handle = handle {
            _ = plugin_reset(handle)
            renderStatePtr.pointee.handle = handle
            renderStatePtr.pointee.channels = channels
            rustSampleRate = sampleRate
        } else {
            let error = plugin_get_last_error()
            let msg = error != nil ? String(cString: error!) : "Unknown error"
            NSLog("SOTF: Failed to create \(pluginType) plugin (\(channels)ch, \(sampleRate)Hz): \(msg)")
        }
    }

    // MARK: - Parameter Tree

    private func buildParameterTree() {
        guard let handle = renderStatePtr.pointee.handle else { return }

        let paramCount = plugin_get_parameter_count(handle)
        guard paramCount > 0 else { return }

        var params: [AUParameter] = []

        for i in 0..<Int(paramCount) {
            guard let info = plugin_get_parameter_info(handle, i) else { continue }

            let paramId = String(cString: info.pointee.id)
            let paramName = String(cString: info.pointee.name)
            let unitStr = String(cString: info.pointee.unit)

            let auUnit: AudioUnitParameterUnit
            switch unitStr {
            case "Hz": auUnit = .hertz
            case "dB": auUnit = .decibels
            case "ms": auUnit = .milliseconds
            case "%": auUnit = .percent
            default: auUnit = .generic
            }

            let param = AUParameterTree.createParameter(
                withIdentifier: paramId,
                name: paramName,
                address: AUParameterAddress(i),
                min: AUValue(info.pointee.min_value),
                max: AUValue(info.pointee.max_value),
                unit: auUnit,
                unitName: unitStr.isEmpty ? nil : unitStr,
                flags: [.flag_IsReadable, .flag_IsWritable],
                valueStrings: nil,
                dependentParameters: nil
            )
            param.value = AUValue(info.pointee.default_value)
            params.append(param)
        }

        auParameters = params
        _parameterTree = AUParameterTree.createTree(withChildren: params)

        _parameterTree?.implementorValueObserver = { [weak self] param, value in
            self?.syncParameterToRust(param: param, value: value)
        }

        _parameterTree?.implementorValueProvider = { [weak self] param -> AUValue in
            // Never access param.value here — it re-enters this callback (infinite recursion)
            return self?.readParameterFromRust(param: param) ?? param.minValue
        }

        _parameterTree?.implementorStringFromValueCallback = { param, valuePtr in
            let value = valuePtr?.pointee ?? param.minValue
            if param.unit == .hertz {
                return String(format: "%.1f Hz", value)
            } else if param.unit == .decibels {
                return String(format: "%.1f dB", value)
            } else if param.unit == .milliseconds {
                return String(format: "%.1f ms", value)
            } else if param.unit == .percent {
                return String(format: "%.0f%%", value)
            }
            if value == value.rounded() && param.maxValue - param.minValue < 1000 {
                return String(format: "%.0f", value)
            }
            return String(format: "%.2f", value)
        }
    }

    private func syncParameterToRust(param: AUParameter, value: AUValue) {
        guard let handle = renderStatePtr.pointee.handle else { return }

        let paramId = param.identifier
        let normalized = Double(normalize(value: value, param: param))

        _ = paramId.withCString { idPtr in
            plugin_set_parameter(handle, idPtr, normalized)
        }
    }

    private func readParameterFromRust(param: AUParameter) -> AUValue {
        // Never access param.value in this method — it triggers implementorValueProvider
        // which calls back into this method, causing infinite recursion.
        guard let handle = renderStatePtr.pointee.handle else { return param.minValue }

        let paramId = param.identifier
        let normalized = paramId.withCString { idPtr in
            plugin_get_parameter(handle, idPtr)
        }

        if normalized < 0 { return param.minValue }
        return denormalize(normalized: Float(normalized), param: param)
    }

    private func normalize(value: AUValue, param: AUParameter) -> Float {
        let range = param.maxValue - param.minValue
        guard range > 0 else { return 0 }
        // Logarithmic scaling for frequency parameters (must match Rust ParamBridge)
        if param.unit == .hertz && param.minValue > 0 {
            let logMin = log(param.minValue)
            let logMax = log(param.maxValue)
            let logVal = log(max(value, param.minValue))
            return (logVal - logMin) / (logMax - logMin)
        }
        return (value - param.minValue) / range
    }

    private func denormalize(normalized: Float, param: AUParameter) -> AUValue {
        // Logarithmic scaling for frequency parameters (must match Rust ParamBridge)
        if param.unit == .hertz && param.minValue > 0 {
            let logMin = log(param.minValue)
            let logMax = log(param.maxValue)
            return exp(logMin + normalized * (logMax - logMin))
        }
        return param.minValue + normalized * (param.maxValue - param.minValue)
    }

    // MARK: - AUAudioUnit Overrides

    public override var parameterTree: AUParameterTree? {
        get { return _parameterTree }
        set { _parameterTree = newValue }
    }

    public override var inputBusses: AUAudioUnitBusArray { return _inputBusArray }
    public override var outputBusses: AUAudioUnitBusArray { return _outputBusArray }

    public override var maximumFramesToRender: AUAudioFrameCount {
        get { return _maxFramesToRender }
        set { _maxFramesToRender = newValue }
    }

    /// All current AU plugins are in-place effects: any channel count, input == output.
    /// [-1, -1] means "any N channels, same on input and output".
    public override var channelCapabilities: [NSNumber]? {
        return [NSNumber(value: -1), NSNumber(value: -1)]
    }

    public override func allocateRenderResources() throws {
        try super.allocateRenderResources()

        // Read the actual format the host has set on our buses
        let channels = Int(inputBus.format.channelCount)
        let sampleRate = UInt32(inputBus.format.sampleRate)

        // Re-create Rust plugin if format changed
        if channels != renderStatePtr.pointee.channels || sampleRate != rustSampleRate {
            createRustPlugin(channels: channels, sampleRate: sampleRate)
        }

        // Pre-allocate scratch buffers for interleave/deinterleave
        let maxFrames = Int(maximumFramesToRender)
        let needed = maxFrames * max(channels, 1)

        if needed > renderStatePtr.pointee.scratchCapacity {
            renderStatePtr.pointee.scratchIn?.deallocate()
            renderStatePtr.pointee.scratchOut?.deallocate()
            renderStatePtr.pointee.scratchIn = .allocate(capacity: needed)
            renderStatePtr.pointee.scratchOut = .allocate(capacity: needed)
            renderStatePtr.pointee.scratchCapacity = needed
        }

        let eventCapacity = 256
        if renderStatePtr.pointee.eventCapacity < eventCapacity {
            renderStatePtr.pointee.midiIn?.deallocate()
            renderStatePtr.pointee.midiOut?.deallocate()
            renderStatePtr.pointee.noteExpressionOut?.deallocate()
            renderStatePtr.pointee.midiIn = .allocate(capacity: eventCapacity)
            renderStatePtr.pointee.midiOut = .allocate(capacity: eventCapacity)
            renderStatePtr.pointee.noteExpressionOut = .allocate(capacity: eventCapacity)
            renderStatePtr.pointee.eventCapacity = eventCapacity
        }
    }

    public override func deallocateRenderResources() {
        super.deallocateRenderResources()
    }

    // MARK: - Audio Processing

    public override var internalRenderBlock: AUInternalRenderBlock {
        // Capture the POINTER, not the values. The pointed-to struct is updated
        // by allocateRenderResources, so the render thread always sees current state.
        let statePtr = renderStatePtr
        let midiOutputBlock = midiOutputEventBlock

        return { (
            actionFlags,
            timestamp,
            frameCount,
            outputBusNumber,
            outputData,
            realtimeEventListHead,
            pullInputBlock
        ) in
            let state = statePtr.pointee
            guard let pullInputBlock = pullInputBlock, let handle = state.handle else {
                return kAudioUnitErr_NoConnection
            }

            let channels = state.channels
            guard channels > 0 else {
                return kAudioUnitErr_Uninitialized
            }

            // Pull input audio
            var pullFlags = AudioUnitRenderActionFlags(rawValue: 0)
            let status = pullInputBlock(&pullFlags, timestamp, frameCount, 0, outputData)
            guard status == noErr else { return status }

            guard let scratchIn = state.scratchIn, let scratchOut = state.scratchOut else {
                return kAudioUnitErr_Uninitialized
            }

            let frames = Int(frameCount)
            let outputBufferList = UnsafeMutableAudioBufferListPointer(outputData)

            // Interleave input from AU's deinterleaved buffers
            if outputBufferList.count == 1 && outputBufferList[0].mNumberChannels == UInt32(channels) {
                if let mData = outputBufferList[0].mData {
                    let src = mData.assumingMemoryBound(to: Float.self)
                    scratchIn.update(from: src, count: frames * channels)
                }
            } else {
                let bufCount = min(Int(outputBufferList.count), channels)
                for ch in 0..<bufCount {
                    guard let mData = outputBufferList[ch].mData else { continue }
                    let src = mData.assumingMemoryBound(to: Float.self)
                    for frame in 0..<frames {
                        scratchIn[frame * channels + ch] = src[frame]
                    }
                }
            }

            let midiInCount = Self.copyMIDIInputEvents(
                from: realtimeEventListHead,
                to: state.midiIn,
                capacity: state.eventCapacity,
                frameCount: frames
            )

            // Process through Rust plugin and copy any queued MIDI/Note Expression output events.
            var midiOutCount = 0
            var noteExpressionOutCount = 0
            let result = plugin_process_with_events(
                handle,
                scratchIn,
                scratchOut,
                frames,
                state.midiIn,
                midiInCount,
                nil,
                0,
                state.midiOut,
                state.eventCapacity,
                &midiOutCount,
                state.noteExpressionOut,
                state.eventCapacity,
                &noteExpressionOutCount
            )
            guard result == 0 else { return OSStatus(kAudioUnitErr_FailedInitialization) }

            if let midiOutputBlock = midiOutputBlock, let midiOut = state.midiOut {
                for i in 0..<midiOutCount {
                    var event = midiOut[i]
                    withUnsafeBytes(of: &event.data) { bytes in
                        if let base = bytes.baseAddress {
                            _ = midiOutputBlock(
                                AUEventSampleTime(event.sample_offset),
                                0,
                                Int(event.len),
                                base.assumingMemoryBound(to: UInt8.self)
                            )
                        }
                    }
                }
            }

            // Deinterleave output back to AU's buffers
            if outputBufferList.count == 1 && outputBufferList[0].mNumberChannels == UInt32(channels) {
                if let mData = outputBufferList[0].mData {
                    let dst = mData.assumingMemoryBound(to: Float.self)
                    dst.update(from: scratchOut, count: frames * channels)
                }
            } else {
                let bufCount = min(Int(outputBufferList.count), channels)
                for ch in 0..<bufCount {
                    guard let mData = outputBufferList[ch].mData else { continue }
                    let dst = mData.assumingMemoryBound(to: Float.self)
                    for frame in 0..<frames {
                        dst[frame] = scratchOut[frame * channels + ch]
                    }
                }
            }

            return noErr
        }
    }

    // MARK: - State Management

    public static var sotfPresetTypeIdentifier: String {
        let info = plugin_preset_document_info()
        return info.ut_type.map { String(cString: $0) } ?? "org.spinorama.sotf.plugin-preset"
    }

    @available(macOS 11.0, iOS 14.0, *)
    public static var sotfPresetType: UTType {
        UTType(exportedAs: sotfPresetTypeIdentifier)
    }

    public override var supportsMPE: Bool {
        plugin_ffi_capabilities().supports_note_expression
    }

    public override var midiOutputNames: [String] {
        plugin_ffi_capabilities().supports_midi_output ? ["MIDI Output"] : []
    }

    public override var fullState: [String: Any]? {
        get {
            guard let handle = renderStatePtr.pointee.handle else { return nil }

            var len: Int = 0
            guard let data = plugin_save_state(handle, &len), len > 0 else { return nil }
            defer { plugin_free_state(data, len) }

            let buffer = Data(bytes: data, count: len)

            var state: [String: Any] = [
                kAUPresetTypeKey: FourCharCode(kAudioUnitType_Effect) as NSNumber,
                kAUPresetSubtypeKey: fourCharCode(type(of: self).pluginSubtype) as NSNumber,
                kAUPresetManufacturerKey: fourCharCode("SOTF") as NSNumber,
                kAUPresetVersionKey: 1 as NSNumber,
                kAUPresetNameKey: "Default",
                "sotf_state": buffer,
            ]

            for param in auParameters {
                state[param.identifier] = param.value
            }

            return state
        }
        set {
            guard let state = newValue, let _ = renderStatePtr.pointee.handle else { return }

            if let data = state["sotf_state"] as? Data {
                data.withUnsafeBytes { bytes in
                    guard let ptr = bytes.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return }
                    plugin_load_state(renderStatePtr.pointee.handle, ptr, bytes.count)
                }

                for param in auParameters {
                    param.value = readParameterFromRust(param: param)
                }
                return
            }

            for param in auParameters {
                if let value = state[param.identifier] as? Float {
                    param.value = value
                    syncParameterToRust(param: param, value: value)
                }
            }
        }
    }

    public override var fullStateForDocument: [String: Any]? {
        get {
            guard var state = fullState else { return nil }
            let documentInfo = plugin_preset_document_info()
            state["sotf_preset_schema_version"] = NSNumber(value: documentInfo.schema_version)
            if let utType = documentInfo.ut_type {
                state["sotf_preset_ut_type"] = String(cString: utType)
            }
            if let fileExtension = documentInfo.file_extension {
                state["sotf_preset_file_extension"] = String(cString: fileExtension)
            }
            state["sotf_plugin_type"] = type(of: self).pluginType
            return state
        }
        set {
            fullState = newValue
        }
    }

    public func exportPreset(named name: String) -> Data? {
        guard let handle = renderStatePtr.pointee.handle else { return nil }
        var len = 0
        let ptr = name.withCString { namePtr in
            plugin_export_preset_json(handle, namePtr, &len)
        }
        guard let ptr = ptr, len > 0 else { return nil }
        defer { plugin_free_state(ptr, len) }
        return Data(bytes: ptr, count: len)
    }

    public func importPresetDocument(_ data: Data) -> Bool {
        guard let handle = renderStatePtr.pointee.handle else { return false }
        return data.withUnsafeBytes { bytes in
            guard let ptr = bytes.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return false }
            return plugin_import_preset_json(handle, ptr, bytes.count) == 0
        }
    }

    public func suggestedPresetFilename(named name: String) -> String? {
        guard let handle = renderStatePtr.pointee.handle else { return nil }
        let ptr = name.withCString { namePtr in
            plugin_suggest_preset_filename(handle, namePtr)
        }
        guard let ptr = ptr else { return nil }
        defer { plugin_free_string(ptr) }
        return String(cString: ptr)
    }

    #if os(macOS)
    public func makePresetBookmark(for url: URL) throws -> Data {
        try url.bookmarkData(options: [.withSecurityScope],
                             includingResourceValuesForKeys: nil,
                             relativeTo: nil)
    }

    public func resolvePresetBookmark(_ data: Data, stale: inout Bool) throws -> URL {
        try URL(resolvingBookmarkData: data,
                options: [.withSecurityScope],
                relativeTo: nil,
                bookmarkDataIsStale: &stale)
    }
    #endif

    private static func copyMIDIInputEvents(
        from eventList: UnsafePointer<AURenderEvent>?,
        to buffer: UnsafeMutablePointer<PluginMidiEvent>?,
        capacity: Int,
        frameCount: Int
    ) -> Int {
        guard let buffer = buffer, capacity > 0 else { return 0 }

        var count = 0
        var event = eventList
        while let current = event, count < capacity {
            let head = current.pointee.head
            if head.eventType == .MIDI {
                let midi = current.pointee.MIDI
                if midi.length > 0 && midi.length <= 3 {
                    let maxOffset = max(frameCount - 1, 0)
                    let rawOffset = midi.eventSampleTime
                    let sampleOffset: Int
                    if rawOffset <= 0 {
                        sampleOffset = 0
                    } else if rawOffset >= AUEventSampleTime(maxOffset) {
                        sampleOffset = maxOffset
                    } else {
                        sampleOffset = Int(rawOffset)
                    }
                    buffer[count] = PluginMidiEvent(
                        sample_offset: sampleOffset,
                        data: (midi.data.0, midi.data.1, midi.data.2),
                        len: UInt8(midi.length)
                    )
                    count += 1
                }
            }
            event = UnsafePointer(head.next)
        }
        return count
    }
}

// MARK: - Helper Functions

private func fourCharCode(_ string: String) -> FourCharCode {
    var result: FourCharCode = 0
    for char in string.prefix(4).utf8 {
        result = result << 8 + FourCharCode(char)
    }
    return result
}

private let kAUPresetTypeKey = "type"
private let kAUPresetSubtypeKey = "subtype"
private let kAUPresetManufacturerKey = "manufacturer"
private let kAUPresetVersionKey = "version"
private let kAUPresetNameKey = "name"
