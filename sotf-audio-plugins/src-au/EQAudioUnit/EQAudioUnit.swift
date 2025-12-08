// EQAudioUnit.swift
// SOTF Parametric EQ Audio Unit

import AVFoundation
import AudioToolbox
import CoreAudioKit

/// SOTF Parametric EQ Audio Unit
public class EQAudioUnit: AUAudioUnit {
    // MARK: - Properties

    /// Rust plugin handle
    private var pluginHandle: OpaquePointer?

    /// Audio format
    private var inputFormat: AVAudioFormat
    private var outputFormat: AVAudioFormat

    /// Input/output busses
    private var inputBus: AUAudioUnitBus
    private var outputBus: AUAudioUnitBus
    private var inputBusArray: AUAudioUnitBusArray!
    private var outputBusArray: AUAudioUnitBusArray!

    /// AU parameters
    private var auParameters: [AUParameter] = []
    private var _parameterTree: AUParameterTree!

    /// Processing state
    private var maxFramesToRender: UInt32 = 512

    // MARK: - Initialization

    public override init(componentDescription: AudioComponentDescription,
                        options: AudioComponentInstantiationOptions = []) throws {
        // Create stereo format at 48kHz
        guard let format = AVAudioFormat(standardFormatWithSampleRate: 48000, channels: 2) else {
            throw NSError(domain: NSOSStatusErrorDomain, code: Int(kAudioUnitErr_FormatNotSupported))
        }

        self.inputFormat = format
        self.outputFormat = format

        // Create busses
        do {
            inputBus = try AUAudioUnitBus(format: format)
            outputBus = try AUAudioUnitBus(format: format)
            inputBus.maximumChannelCount = 2
            outputBus.maximumChannelCount = 2
        } catch {
            throw error
        }

        try super.init(componentDescription: componentDescription, options: options)

        // Create bus arrays
        inputBusArray = AUAudioUnitBusArray(audioUnit: self, busType: .input, busses: [inputBus])
        outputBusArray = AUAudioUnitBusArray(audioUnit: self, busType: .output, busses: [outputBus])

        // Initialize Rust plugin
        initializePlugin()

        // Create parameter tree
        createParameterTree()
    }

    deinit {
        if let handle = pluginHandle {
            plugin_destroy(handle)
        }
    }

    // MARK: - Plugin Initialization

    private func initializePlugin() {
        // Default EQ configuration (10-band parametric EQ, all flat)
        let config = """
        {
            "filters": []
        }
        """

        guard let handle = plugin_create(
            "EQ",
            config,
            UInt32(inputFormat.sampleRate),
            Int(inputFormat.channelCount),
            Int(outputFormat.channelCount)
        ) else {
            if let error = plugin_get_last_error() {
                let errorStr = String(cString: error)
                NSLog("Failed to create EQ plugin: \(errorStr)")
            }
            return
        }

        self.pluginHandle = handle
        NSLog("EQ plugin created successfully")
    }

    // MARK: - Parameter Tree

    private func createParameterTree() {
        guard let handle = pluginHandle else {
            NSLog("Cannot create parameter tree: plugin not initialized")
            return
        }

        // Get parameter count from Rust
        let paramCount = plugin_get_parameter_count(handle)
        NSLog("Creating parameter tree with \(paramCount) parameters")

        // Create AU parameters from Rust parameter info
        for i in 0..<Int(paramCount) {
            guard let paramInfo = plugin_get_parameter_info(handle, i) else {
                continue
            }

            let id = String(cString: paramInfo.pointee.id)
            let name = String(cString: paramInfo.pointee.name)
            let unit = String(cString: paramInfo.pointee.unit)

            // Convert unit string to AudioUnitParameterUnit
            let auUnit: AudioUnitParameterUnit
            switch unit {
            case "Hz":
                auUnit = .hertz
            case "dB":
                auUnit = .decibels
            default:
                auUnit = .generic
            }

            // Create AU parameter (using index as address)
            let param = AUParameterTree.createParameter(
                withIdentifier: id,
                name: name,
                address: AUParameterAddress(i),
                min: Float(paramInfo.pointee.min_value),
                max: Float(paramInfo.pointee.max_value),
                unit: auUnit,
                unitName: unit.isEmpty ? nil : unit,
                flags: [.flag_IsReadable, .flag_IsWritable],
                valueStrings: nil,
                dependentParameters: nil
            )

            // Set default value
            param.value = Float(paramInfo.pointee.default_value)

            auParameters.append(param)
        }

        // Create parameter tree
        _parameterTree = AUParameterTree.createTree(withChildren: auParameters)

        // Set up parameter observation
        _parameterTree.implementorValueObserver = { [weak self] param, value in
            self?.setParameterValue(param: param, value: value)
        }

        _parameterTree.implementorValueProvider = { [weak self] param in
            return self?.getParameterValue(param: param) ?? param.value
        }
    }

    private func setParameterValue(param: AUParameter, value: AUValue) {
        guard let handle = pluginHandle else { return }

        let id = param.identifier

        // Convert raw value to normalized (0.0-1.0)
        let normalized = Double((value - param.minValue) / (param.maxValue - param.minValue))

        // Set in Rust plugin
        let result = plugin_set_parameter(handle, id, normalized)
        if result != 0 {
            NSLog("Failed to set parameter \(id): error \(result)")
        }
    }

    private func getParameterValue(param: AUParameter) -> AUValue {
        guard let handle = pluginHandle else { return param.value }

        let id = param.identifier

        // Get normalized value from Rust
        let normalized = plugin_get_parameter(handle, id)

        if normalized < 0 {
            return param.value // Error, return cached value
        }

        // Denormalize to raw value
        let value = param.minValue + Float(normalized) * (param.maxValue - param.minValue)
        return value
    }

    // MARK: - AUAudioUnit Overrides

    public override var parameterTree: AUParameterTree? {
        get {
            return _parameterTree
        }
        set {
            _parameterTree = newValue
        }
    }

    public override var inputBusses: AUAudioUnitBusArray {
        return inputBusArray
    }

    public override var outputBusses: AUAudioUnitBusArray {
        return outputBusArray
    }

    public override var maximumFramesToRender: AUAudioFrameCount {
        get { return maxFramesToRender }
        set { maxFramesToRender = newValue }
    }

    public override func allocateRenderResources() throws {
        try super.allocateRenderResources()

        // Update plugin sample rate if format changed
        if let handle = pluginHandle {
            let newSampleRate = UInt32(outputBus.format.sampleRate)
            plugin_reset(handle)
            NSLog("Allocated render resources at \(newSampleRate)Hz")
        }
    }

    public override func deallocateRenderResources() {
        super.deallocateRenderResources()

        // Reset plugin state
        if let handle = pluginHandle {
            plugin_reset(handle)
        }
    }

    // MARK: - Audio Processing

    public override var internalRenderBlock: AUInternalRenderBlock {
        return { [weak self] (
            actionFlags,
            timestamp,
            frameCount,
            outputBusNumber,
            outputData,
            realtimeEventListHead,
            pullInputBlock
        ) in
            guard let self = self else {
                return kAudioUnitErr_Uninitialized
            }

            guard let handle = self.pluginHandle else {
                return kAudioUnitErr_Uninitialized
            }

            guard let pullInputBlock = pullInputBlock else {
                return kAudioUnitErr_NoConnection
            }

            // Pull input audio
            var pullFlags = AudioUnitRenderActionFlags(rawValue: 0)
            let status = pullInputBlock(&pullFlags, timestamp, frameCount, 0, outputData)

            guard status == noErr else {
                return status
            }

            // Get audio buffer
            let outputBufferList = UnsafeMutableAudioBufferListPointer(outputData)
            guard let mData = outputBufferList[0].mData else {
                return kAudioUnitErr_NoConnection
            }

            let inputPtr = mData.assumingMemoryBound(to: Float.self)
            let outputPtr = mData.assumingMemoryBound(to: Float.self)

            // Process through Rust plugin
            let result = plugin_process(handle, inputPtr, outputPtr, Int(frameCount))

            if result != 0 {
                // Processing failed - pass through audio unchanged
                return noErr
            }

            return noErr
        }
    }

    // MARK: - State Management

    public override var fullState: [String: Any]? {
        get {
            var state: [String: Any] = [:]

            // Save all parameter values
            for param in auParameters {
                state[param.identifier] = param.value
            }

            return state
        }
        set {
            guard let state = newValue else { return }

            // Restore parameter values
            for param in auParameters {
                if let value = state[param.identifier] as? Float {
                    param.value = value
                    setParameterValue(param: param, value: value)
                }
            }
        }
    }
}

// MARK: - Factory Function

/// Factory function required by Audio Unit extension
/// Uses C-linkage to ensure it's callable from Objective-C runtime
@_cdecl("EQAudioUnitFactory")
public func EQAudioUnitFactory(componentDescription: UnsafePointer<AudioComponentDescription>) -> UnsafeMutableRawPointer? {
    do {
        let audioUnit = try EQAudioUnit(componentDescription: componentDescription.pointee, options: [])
        return Unmanaged.passRetained(audioUnit).toOpaque()
    } catch {
        NSLog("Failed to create EQAudioUnit: \(error)")
        return nil
    }
}
