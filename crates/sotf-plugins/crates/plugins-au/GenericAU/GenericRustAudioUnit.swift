// GenericRustAudioUnit.swift
// Base class for all SOTF Audio Units that delegate to Rust plugins.
//
// Subclasses only need to override pluginType and pluginSubtype.

import AVFoundation
import AudioToolbox
import CoreAudioKit

// MARK: - Render State (shared between main thread and render thread via pointer)

/// Mutable render state shared with the real-time render block via UnsafeMutablePointer.
/// The render block captures a pointer to this; allocateRenderResources updates the fields.
private struct RenderState {
    var handle: OpaquePointer?
    var channels: Int
    var scratchIn: UnsafeMutablePointer<Float>?
    var scratchOut: UnsafeMutablePointer<Float>?
    var scratchCapacity: Int
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

    // MARK: - Initialization

    public override init(componentDescription: AudioComponentDescription,
                        options: AudioComponentInstantiationOptions = []) throws {
        // Allocate render state on the heap
        renderStatePtr = .allocate(capacity: 1)
        renderStatePtr.initialize(to: RenderState(
            handle: nil, channels: 0, scratchIn: nil, scratchOut: nil, scratchCapacity: 0
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

        paramId.withCString { idPtr in
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
        return (value - param.minValue) / range
    }

    private func denormalize(normalized: Float, param: AUParameter) -> AUValue {
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
    }

    public override func deallocateRenderResources() {
        super.deallocateRenderResources()
    }

    // MARK: - Audio Processing

    public override var internalRenderBlock: AUInternalRenderBlock {
        // Capture the POINTER, not the values. The pointed-to struct is updated
        // by allocateRenderResources, so the render thread always sees current state.
        let statePtr = renderStatePtr

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

            // Process through Rust plugin
            let result = plugin_process(handle, scratchIn, scratchOut, frames)
            guard result == 0 else { return OSStatus(kAudioUnitErr_FailedInitialization) }

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
