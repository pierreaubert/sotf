// GenericRustAudioUnit.swift
// Base class for all SOTF Audio Units that delegate to Rust plugins.
//
// Subclasses only need to override pluginType and pluginSubtype.

import AVFoundation
import AudioToolbox
import CoreAudioKit

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

    /// Number of input channels
    open class var inputChannelCount: UInt32 { 2 }

    /// Number of output channels
    open class var outputChannelCount: UInt32 { 2 }

    // MARK: - Properties

    private var rustHandle: OpaquePointer?
    private var inputFormat: AVAudioFormat
    private var outputFormat: AVAudioFormat
    private var inputBus: AUAudioUnitBus
    private var outputBus: AUAudioUnitBus
    private var _inputBusArray: AUAudioUnitBusArray!
    private var _outputBusArray: AUAudioUnitBusArray!
    private var _parameterTree: AUParameterTree?
    private var auParameters: [AUParameter] = []
    private var _maxFramesToRender: UInt32 = 512

    /// Pre-allocated scratch buffers for interleave/deinterleave
    private var scratchInput: UnsafeMutablePointer<Float>?
    private var scratchOutput: UnsafeMutablePointer<Float>?
    private var scratchCapacity: Int = 0

    // MARK: - Initialization

    public override init(componentDescription: AudioComponentDescription,
                        options: AudioComponentInstantiationOptions = []) throws {
        let inCh = type(of: self).inputChannelCount
        let outCh = type(of: self).outputChannelCount

        guard let format = AVAudioFormat(standardFormatWithSampleRate: 48000, channels: inCh) else {
            throw NSError(domain: NSOSStatusErrorDomain, code: Int(kAudioUnitErr_FormatNotSupported))
        }

        self.inputFormat = format
        self.outputFormat = format

        inputBus = try AUAudioUnitBus(format: format)
        outputBus = try AUAudioUnitBus(format: format)
        inputBus.maximumChannelCount = inCh
        outputBus.maximumChannelCount = outCh

        try super.init(componentDescription: componentDescription, options: options)

        _inputBusArray = AUAudioUnitBusArray(audioUnit: self, busType: .input, busses: [inputBus])
        _outputBusArray = AUAudioUnitBusArray(audioUnit: self, busType: .output, busses: [outputBus])

        // Create Rust plugin
        let pluginType = type(of: self).pluginType
        let sampleRate = UInt32(format.sampleRate)

        rustHandle = pluginType.withCString { typePtr in
            "{}".withCString { configPtr in
                plugin_create(typePtr, configPtr, sampleRate, Int(inCh), Int(outCh))
            }
        }

        if rustHandle == nil {
            let error = plugin_get_last_error()
            let msg = error != nil ? String(cString: error!) : "Unknown error"
            NSLog("SOTF: Failed to create \(pluginType) plugin: \(msg)")
        }

        buildParameterTree()
    }

    deinit {
        if let handle = rustHandle {
            plugin_destroy(handle)
        }
        if let scratch = scratchInput {
            scratch.deallocate()
        }
        if let scratch = scratchOutput {
            scratch.deallocate()
        }
    }

    // MARK: - Parameter Tree

    private func buildParameterTree() {
        guard let handle = rustHandle else { return }

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

        // Parameter observation: sync to Rust plugin
        _parameterTree?.implementorValueObserver = { [weak self] param, value in
            self?.syncParameterToRust(param: param, value: value)
        }

        _parameterTree?.implementorValueProvider = { [weak self] param -> AUValue in
            return self?.readParameterFromRust(param: param) ?? param.value
        }
    }

    private func syncParameterToRust(param: AUParameter, value: AUValue) {
        guard let handle = rustHandle else { return }

        let paramId = param.identifier
        let normalized = Double(normalize(value: value, param: param))

        paramId.withCString { idPtr in
            plugin_set_parameter(handle, idPtr, normalized)
        }
    }

    private func readParameterFromRust(param: AUParameter) -> AUValue {
        guard let handle = rustHandle else { return param.value }

        let paramId = param.identifier
        let normalized = paramId.withCString { idPtr in
            plugin_get_parameter(handle, idPtr)
        }

        if normalized < 0 { return param.value }
        return denormalize(normalized: Float(normalized), param: param)
    }

    /// Normalize a raw value to 0-1 based on the parameter's min/max
    private func normalize(value: AUValue, param: AUParameter) -> Float {
        let range = param.maxValue - param.minValue
        guard range > 0 else { return 0 }
        return (value - param.minValue) / range
    }

    /// Denormalize a 0-1 value to the parameter's min/max range
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

    public override var channelCapabilities: [NSNumber]? {
        let inCh = type(of: self).inputChannelCount
        let outCh = type(of: self).outputChannelCount
        return [NSNumber(value: inCh), NSNumber(value: outCh)]
    }

    public override func allocateRenderResources() throws {
        try super.allocateRenderResources()

        // Re-initialize Rust plugin with actual sample rate
        if let handle = rustHandle {
            let sr = UInt32(inputBus.format.sampleRate)
            plugin_reset(handle)
            // plugin_reset clears state; sample rate is set at creation
        }

        // Pre-allocate scratch buffers
        let maxFrames = Int(maximumFramesToRender)
        let inCh = Int(type(of: self).inputChannelCount)
        let outCh = Int(type(of: self).outputChannelCount)
        let maxCh = max(inCh, outCh)
        let needed = maxFrames * maxCh

        if needed > scratchCapacity {
            scratchInput?.deallocate()
            scratchOutput?.deallocate()
            scratchInput = .allocate(capacity: needed)
            scratchOutput = .allocate(capacity: needed)
            scratchCapacity = needed
        }
    }

    public override func deallocateRenderResources() {
        super.deallocateRenderResources()
    }

    // MARK: - Audio Processing

    public override var internalRenderBlock: AUInternalRenderBlock {
        let handle = rustHandle
        let inCh = Int(type(of: self).inputChannelCount)
        let outCh = Int(type(of: self).outputChannelCount)
        let scratchIn = scratchInput
        let scratchOut = scratchOutput

        return { (
            actionFlags,
            timestamp,
            frameCount,
            outputBusNumber,
            outputData,
            realtimeEventListHead,
            pullInputBlock
        ) in
            guard let pullInputBlock = pullInputBlock, let handle = handle else {
                return kAudioUnitErr_NoConnection
            }

            // Pull input audio
            var pullFlags = AudioUnitRenderActionFlags(rawValue: 0)
            let status = pullInputBlock(&pullFlags, timestamp, frameCount, 0, outputData)
            guard status == noErr else { return status }

            guard let scratchIn = scratchIn, let scratchOut = scratchOut else {
                return kAudioUnitErr_Uninitialized
            }

            let frames = Int(frameCount)
            let outputBufferList = UnsafeMutableAudioBufferListPointer(outputData)

            // Interleave input from AU's non-interleaved buffers
            if outputBufferList.count == 1 && outputBufferList[0].mNumberChannels == UInt32(inCh) {
                // Already interleaved (some hosts do this)
                if let mData = outputBufferList[0].mData {
                    let src = mData.assumingMemoryBound(to: Float.self)
                    scratchIn.update(from: src, count: frames * inCh)
                }
            } else {
                // Non-interleaved: interleave manually
                for ch in 0..<min(Int(outputBufferList.count), inCh) {
                    guard let mData = outputBufferList[ch].mData else { continue }
                    let src = mData.assumingMemoryBound(to: Float.self)
                    for frame in 0..<frames {
                        scratchIn[frame * inCh + ch] = src[frame]
                    }
                }
            }

            // Process through Rust plugin
            let result = plugin_process(handle, scratchIn, scratchOut, frames)
            guard result == 0 else { return OSStatus(kAudioUnitErr_FailedInitialization) }

            // Deinterleave output back to AU's non-interleaved buffers
            if outputBufferList.count == 1 && outputBufferList[0].mNumberChannels == UInt32(outCh) {
                // Already interleaved
                if let mData = outputBufferList[0].mData {
                    let dst = mData.assumingMemoryBound(to: Float.self)
                    dst.update(from: scratchOut, count: frames * outCh)
                }
            } else {
                // Non-interleaved: deinterleave
                for ch in 0..<min(Int(outputBufferList.count), outCh) {
                    guard let mData = outputBufferList[ch].mData else { continue }
                    let dst = mData.assumingMemoryBound(to: Float.self)
                    for frame in 0..<frames {
                        dst[frame] = scratchOut[frame * outCh + ch]
                    }
                }
            }

            return noErr
        }
    }

    // MARK: - State Management

    public override var fullState: [String: Any]? {
        get {
            guard let handle = rustHandle else { return nil }

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

            // Also store individual parameter values for host compatibility
            for param in auParameters {
                state[param.identifier] = param.value
            }

            return state
        }
        set {
            guard let state = newValue, let handle = rustHandle else { return }

            // Try SOTF state blob first
            if let data = state["sotf_state"] as? Data {
                data.withUnsafeBytes { bytes in
                    guard let ptr = bytes.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return }
                    plugin_load_state(handle, ptr, bytes.count)
                }

                // Sync AU parameters from Rust state
                for param in auParameters {
                    param.value = readParameterFromRust(param: param)
                }
                return
            }

            // Fallback: load individual parameter values
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
