// EQAudioUnit.swift
// SOTF Parametric EQ Audio Unit - Pure Swift Implementation
// Note: This is a standalone Swift implementation that doesn't depend on Rust FFI

import AVFoundation
import AudioToolbox
import CoreAudioKit

/// Number of EQ bands
private let kNumBands = 5

/// SOTF Parametric EQ Audio Unit
public class EQAudioUnit: AUAudioUnit {
    // MARK: - Properties

    /// Audio format
    private var inputFormat: AVAudioFormat
    private var outputFormat: AVAudioFormat

    /// Input/output busses
    private var inputBus: AUAudioUnitBus
    private var outputBus: AUAudioUnitBus
    private var _inputBusArray: AUAudioUnitBusArray!
    private var _outputBusArray: AUAudioUnitBusArray!

    /// AU parameters
    private var auParameters: [AUParameter] = []
    private var _parameterTree: AUParameterTree?

    /// Processing state
    private var _maxFramesToRender: UInt32 = 512

    /// Filter coefficients (biquad) for each band
    private var filterCoefficients: [[Double]] = []

    /// Filter states (z-1, z-2 for each channel, for each band)
    private var filterStates: [[[Double]]] = []

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
        inputBus = try AUAudioUnitBus(format: format)
        outputBus = try AUAudioUnitBus(format: format)
        inputBus.maximumChannelCount = 2
        outputBus.maximumChannelCount = 2

        // Initialize filter states
        filterCoefficients = Array(repeating: [1.0, 0.0, 0.0, 0.0, 0.0], count: kNumBands)
        filterStates = Array(repeating: Array(repeating: [0.0, 0.0], count: 2), count: kNumBands)

        try super.init(componentDescription: componentDescription, options: options)

        // Create bus arrays
        _inputBusArray = AUAudioUnitBusArray(audioUnit: self, busType: .input, busses: [inputBus])
        _outputBusArray = AUAudioUnitBusArray(audioUnit: self, busType: .output, busses: [outputBus])

        // Create parameter tree
        createParameterTree()
    }

    // MARK: - Parameter Tree

    private func createParameterTree() {
        var params: [AUParameter] = []

        // Create parameters for each band: frequency, gain, Q
        for band in 0..<kNumBands {
            let bandNum = band + 1

            // Frequency parameter
            let freq = AUParameterTree.createParameter(
                withIdentifier: "band\(band)_freq",
                name: "Band \(bandNum) Frequency",
                address: AUParameterAddress(band * 3),
                min: 20.0,
                max: 20000.0,
                unit: .hertz,
                unitName: "Hz",
                flags: [.flag_IsReadable, .flag_IsWritable],
                valueStrings: nil,
                dependentParameters: nil
            )
            // Default frequencies spread across spectrum
            let defaultFreqs: [Float] = [100.0, 300.0, 1000.0, 3000.0, 10000.0]
            freq.value = defaultFreqs[band]
            params.append(freq)

            // Gain parameter
            let gain = AUParameterTree.createParameter(
                withIdentifier: "band\(band)_gain",
                name: "Band \(bandNum) Gain",
                address: AUParameterAddress(band * 3 + 1),
                min: -12.0,
                max: 12.0,
                unit: .decibels,
                unitName: "dB",
                flags: [.flag_IsReadable, .flag_IsWritable],
                valueStrings: nil,
                dependentParameters: nil
            )
            gain.value = 0.0
            params.append(gain)

            // Q parameter
            let q = AUParameterTree.createParameter(
                withIdentifier: "band\(band)_q",
                name: "Band \(bandNum) Q",
                address: AUParameterAddress(band * 3 + 2),
                min: 0.1,
                max: 10.0,
                unit: .generic,
                unitName: nil,
                flags: [.flag_IsReadable, .flag_IsWritable],
                valueStrings: nil,
                dependentParameters: nil
            )
            q.value = 1.0
            params.append(q)
        }

        auParameters = params

        // Create parameter tree
        _parameterTree = AUParameterTree.createTree(withChildren: params)

        // Set up parameter observation
        _parameterTree?.implementorValueObserver = { [weak self] param, value in
            self?.parameterChanged(param: param, value: value)
        }

        // Note: implementorValueProvider is NOT set because the AUParameter
        // objects already store their values. Setting it and returning param.value
        // would cause infinite recursion.
    }

    private func parameterChanged(param: AUParameter, value: AUValue) {
        // Recalculate filter coefficients when parameters change
        let band = Int(param.address) / 3

        if band < kNumBands {
            recalculateFilterCoefficients(band: band)
        }
    }

    private func recalculateFilterCoefficients(band: Int) {
        let freqParam = auParameters[band * 3]
        let gainParam = auParameters[band * 3 + 1]
        let qParam = auParameters[band * 3 + 2]

        let freq = Double(freqParam.value)
        let gainDb = Double(gainParam.value)
        let q = Double(qParam.value)
        let sampleRate = inputFormat.sampleRate

        // Calculate peaking EQ biquad coefficients
        let A = pow(10.0, gainDb / 40.0)
        let w0 = 2.0 * Double.pi * freq / sampleRate
        let sinW0 = sin(w0)
        let cosW0 = cos(w0)
        let alpha = sinW0 / (2.0 * q)

        let b0 = 1.0 + alpha * A
        let b1 = -2.0 * cosW0
        let b2 = 1.0 - alpha * A
        let a0 = 1.0 + alpha / A
        let a1 = -2.0 * cosW0
        let a2 = 1.0 - alpha / A

        // Normalize coefficients
        filterCoefficients[band] = [b0/a0, b1/a0, b2/a0, a1/a0, a2/a0]
    }

    // MARK: - AUAudioUnit Overrides

    public override var parameterTree: AUParameterTree? {
        get { return _parameterTree }
        set { _parameterTree = newValue }
    }

    public override var inputBusses: AUAudioUnitBusArray {
        return _inputBusArray
    }

    public override var outputBusses: AUAudioUnitBusArray {
        return _outputBusArray
    }

    public override var maximumFramesToRender: AUAudioFrameCount {
        get { return _maxFramesToRender }
        set { _maxFramesToRender = newValue }
    }

    public override func allocateRenderResources() throws {
        try super.allocateRenderResources()

        // Recalculate all filter coefficients
        for band in 0..<kNumBands {
            recalculateFilterCoefficients(band: band)
        }

        // Reset filter states
        for band in 0..<kNumBands {
            for ch in 0..<2 {
                filterStates[band][ch] = [0.0, 0.0]
            }
        }
    }

    public override func deallocateRenderResources() {
        super.deallocateRenderResources()
    }

    // MARK: - Audio Processing

    public override var internalRenderBlock: AUInternalRenderBlock {
        // Capture necessary state for the render block
        let coefficients = filterCoefficients
        var states = filterStates

        return { (
            actionFlags,
            timestamp,
            frameCount,
            outputBusNumber,
            outputData,
            realtimeEventListHead,
            pullInputBlock
        ) in
            guard let pullInputBlock = pullInputBlock else {
                return kAudioUnitErr_NoConnection
            }

            // Pull input audio
            var pullFlags = AudioUnitRenderActionFlags(rawValue: 0)
            let status = pullInputBlock(&pullFlags, timestamp, frameCount, 0, outputData)

            guard status == noErr else {
                return status
            }

            // Process audio through biquad filters
            let outputBufferList = UnsafeMutableAudioBufferListPointer(outputData)

            for bufferIndex in 0..<outputBufferList.count {
                guard let mData = outputBufferList[bufferIndex].mData else {
                    continue
                }

                let samples = mData.assumingMemoryBound(to: Float.self)
                let numSamples = Int(outputBufferList[bufferIndex].mDataByteSize) / MemoryLayout<Float>.size
                let channelCount = Int(outputBufferList[bufferIndex].mNumberChannels)

                // Process interleaved stereo
                for frame in 0..<(numSamples / channelCount) {
                    for ch in 0..<min(channelCount, 2) {
                        var sample = Double(samples[frame * channelCount + ch])

                        // Apply each band's filter
                        for band in 0..<kNumBands {
                            let c = coefficients[band]
                            let z1 = states[band][ch][0]
                            let z2 = states[band][ch][1]

                            // Direct Form II Transposed biquad
                            let output = c[0] * sample + z1
                            states[band][ch][0] = c[1] * sample - c[3] * output + z2
                            states[band][ch][1] = c[2] * sample - c[4] * output

                            sample = output
                        }

                        samples[frame * channelCount + ch] = Float(sample)
                    }
                }
            }

            return noErr
        }
    }

    // MARK: - Channel Capabilities

    public override var channelCapabilities: [NSNumber]? {
        // Return explicit channel configuration: only stereo (2 in, 2 out)
        return [2, 2]
    }

    // MARK: - State Management

    public override var fullState: [String: Any]? {
        get {
            var state: [String: Any] = [
                kAUPresetTypeKey: FourCharCode(kAudioUnitType_Effect) as NSNumber,
                kAUPresetSubtypeKey: FourCharCode(fourCharCodeFrom("SOEQ")) as NSNumber,
                kAUPresetManufacturerKey: FourCharCode(fourCharCodeFrom("SOTF")) as NSNumber,
                kAUPresetVersionKey: 1 as NSNumber,
                kAUPresetNameKey: "Default"
            ]

            // Add parameter values
            for param in auParameters {
                state[param.identifier] = param.value
            }
            return state
        }
        set {
            guard let state = newValue else { return }
            for param in auParameters {
                if let value = state[param.identifier] as? Float {
                    param.value = value
                }
            }
        }
    }
}

// MARK: - Helper Functions

private func fourCharCodeFrom(_ string: String) -> FourCharCode {
    var result: FourCharCode = 0
    for char in string.prefix(4).utf8 {
        result = result << 8 + FourCharCode(char)
    }
    return result
}

// Preset keys
private let kAUPresetTypeKey = "type"
private let kAUPresetSubtypeKey = "subtype"
private let kAUPresetManufacturerKey = "manufacturer"
private let kAUPresetVersionKey = "version"
private let kAUPresetNameKey = "name"
