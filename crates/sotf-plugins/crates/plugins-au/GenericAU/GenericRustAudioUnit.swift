// GenericRustAudioUnit.swift
// Base class for all SOTF Audio Units that delegate to Rust plugins.
//
// Subclasses only need to override pluginType and pluginSubtype.

import AVFoundation
import AudioToolbox
import CoreAudioKit
import Foundation
import os
import Synchronization
import UniformTypeIdentifiers

#if SOTF_NATIVE_SMOKE
@_silgen_name("sotf_render_counter_enter")
private func sotfRenderCounterEnter()
@_silgen_name("sotf_render_counter_leave")
private func sotfRenderCounterLeave()
#endif

// MARK: - Render State (shared between main thread and render thread via pointer)

/// Mutable render state shared with the real-time render block via UnsafeMutablePointer.
/// The render block captures a pointer to this; allocateRenderResources updates the fields.
private struct RenderState {
    var handle: OpaquePointer?
    var inputChannels: Int
    var outputChannels: Int
    var inputBufferList: UnsafeMutablePointer<AudioBufferList>?
    var scratchIn: UnsafeMutablePointer<Float>?
    var scratchOut: UnsafeMutablePointer<Float>?
    var ownedOutput: UnsafeMutablePointer<Float>?
    var scratchCapacity: Int
    var maximumFrames: UInt32
    var midiIn: UnsafeMutablePointer<PluginMidiEvent>?
    var midiOut: UnsafeMutablePointer<PluginMidiEvent>?
    var noteExpressionOut: UnsafeMutablePointer<PluginNoteExpressionEvent>?
    var eventCapacity: Int
    var parameterIds: UnsafeMutablePointer<UnsafePointer<CChar>?>?
    var parameterMetadata: UnsafeMutablePointer<CachedParameterMetadata>?
    var parameterRamps: UnsafeMutablePointer<ParameterRampState>?
    var parameterMailboxes: UnsafeMutablePointer<Unmanaged<ParameterMailbox>>?
    var parameterCount: Int
    var parameterEpoch: UInt64
    var resourcesAllocated: Bool
}

/// Stack-owned event/ramp segment state. Keeping this out of nested Swift
/// closures avoids `_Block_copy` heap traffic on automated render calls.
private struct RenderSegmentProcessor {
    let state: RenderState
    let handle: OpaquePointer
    let scratchIn: UnsafeMutablePointer<Float>
    let scratchOut: UnsafeMutablePointer<Float>
    let inputChannels: Int
    let outputChannels: Int
    let copiedMIDIInputCount: Int
    let midiOutputEnabled: Bool
    var midiInputCursor = 0
    var midiOutputCount = 0
    var noteExpressionOutputCount = 0

    mutating func processSegment(start: Int, count: Int) -> Int32 {
        guard count > 0 else { return 0 }
        let firstMIDI = midiInputCursor
        if let midiIn = state.midiIn {
            let segmentEnd = start + count
            while midiInputCursor < copiedMIDIInputCount,
                  midiIn[midiInputCursor].sample_offset < segmentEnd {
                midiIn[midiInputCursor].sample_offset -= start
                midiInputCursor += 1
            }
        }
        let midiInCount = midiInputCursor - firstMIDI
        var segmentMidiOutCount = 0
        var segmentNoteOutCount = 0
        let status: Int32
        if midiInCount == 0 && !midiOutputEnabled {
            status = plugin_process(
                handle,
                scratchIn.advanced(by: start * inputChannels),
                scratchOut.advanced(by: start * outputChannels),
                count
            )
        } else {
            status = plugin_process_with_events(
                handle,
                scratchIn.advanced(by: start * inputChannels),
                scratchOut.advanced(by: start * outputChannels),
                count,
                state.midiIn?.advanced(by: firstMIDI),
                midiInCount,
                nil,
                0,
                state.midiOut?.advanced(by: midiOutputCount),
                state.eventCapacity - midiOutputCount,
                &segmentMidiOutCount,
                state.noteExpressionOut?.advanced(by: noteExpressionOutputCount),
                state.eventCapacity - noteExpressionOutputCount,
                &segmentNoteOutCount
            )
        }
        if status == 0 {
            if let midiOut = state.midiOut {
                for index in 0..<segmentMidiOutCount {
                    midiOut[midiOutputCount + index].sample_offset += start
                }
            }
            if let noteOut = state.noteExpressionOut {
                for index in 0..<segmentNoteOutCount {
                    noteOut[noteExpressionOutputCount + index].sample_offset += start
                }
            }
            midiOutputCount += segmentMidiOutCount
            noteExpressionOutputCount += segmentNoteOutCount
        }
        return status
    }

    mutating func processWithActiveRamps(start: Int, count: Int) -> Int32 {
        guard count > 0 else { return 0 }
        guard let ramps = state.parameterRamps, let ids = state.parameterIds else {
            return processSegment(start: start, count: count)
        }
        var active = false
        for address in 0..<state.parameterCount where ramps[address].active {
            active = true
            break
        }
        guard active else { return processSegment(start: start, count: count) }
        var processed = 0
        while processed < count {
            let quantum = min(16, count - processed)
            for address in 0..<state.parameterCount where ramps[address].active {
                var ramp = ramps[address]
                let advance = min(quantum, ramp.remaining)
                ramp.current += ramp.step * Double(advance)
                ramp.remaining -= advance
                if ramp.remaining <= 0 {
                    ramp.current = ramp.target
                    ramp.active = false
                }
                guard let id = ids[address],
                      plugin_set_parameter(handle, id, ramp.current) == 0 else { return -1 }
                ramps[address] = ramp
            }
            guard processSegment(start: start + processed, count: quantum) == 0 else { return -1 }
            processed += quantum
        }
        return 0
    }
}

private struct CachedParameterMetadata {
    var minValue: Double = 0
    var maxValue: Double = 0
    var logarithmic = false
    var steps: UInt32 = 0
    var realtime = false
}

private struct ParameterRampState {
    var active = false
    var current: Double = 0
    var target: Double = 0
    var step: Double = 0
    var remaining = 0
}

/// One lock-free, allocation-free, coalescing publication slot per parameter.
/// Concurrent producers linearize at the successful CAS. Independent
/// parameters commute; repeated writes to one address retain the newest value
/// without overflowing a FIFO or ever waiting on the render thread.
private final class ParameterMailbox {
    private let published = Atomic<UInt64>(0)
    private let appliedVersion = Atomic<UInt32>(0)
    let coalescedCount = Atomic<UInt64>(0)

    @inline(__always)
    func publish(_ value: Float) {
        var current = published.load(ordering: .relaxed)
        while true {
            let version = UInt32(truncatingIfNeeded: current >> 32) &+ 1
            let next = UInt64(version) << 32 | UInt64(value.bitPattern)
            let result = published.compareExchange(
                expected: current,
                desired: next,
                ordering: .acquiringAndReleasing
            )
            if result.exchanged {
                if UInt32(truncatingIfNeeded: current >> 32)
                    != appliedVersion.load(ordering: .acquiring) {
                    coalescedCount.wrappingAdd(1, ordering: .relaxed)
                }
                return
            }
            current = result.original
        }
    }

    @inline(__always)
    func pendingValue() -> (version: UInt32, value: Float)? {
        let word = published.load(ordering: .acquiring)
        let version = UInt32(truncatingIfNeeded: word >> 32)
        guard version != appliedVersion.load(ordering: .acquiring) else { return nil }
        return (version, Float(bitPattern: UInt32(truncatingIfNeeded: word)))
    }

    @inline(__always)
    func markApplied(_ version: UInt32) {
        appliedVersion.store(version, ordering: .releasing)
    }

    @inline(__always)
    func discardPending() {
        let word = published.load(ordering: .acquiring)
        appliedVersion.store(UInt32(truncatingIfNeeded: word >> 32), ordering: .releasing)
    }
}

/// Preallocated single-owner arbitration for the Rust handle and render state.
/// Control callers are given priority once queued, while the render side never
/// waits.
private final class RenderAccessGate {
    private let claimed = Atomic<Bool>(false)
    private let controlWaiters = Atomic<Int>(0)
    private let rendersWhileControlQueued = Atomic<Int>(0)
    private let maximumRenderBurst = 8

    @inline(__always)
    private func tryClaim() -> Bool {
        claimed.compareExchange(
            expected: false,
            desired: true,
            ordering: .acquiring
        ).exchanged
    }

    @inline(__always)
    func tryAcquireRender() -> Bool {
        if controlWaiters.load(ordering: .acquiring) > 0
            && rendersWhileControlQueued.load(ordering: .relaxed) >= maximumRenderBurst {
            return false
        }
        let acquired = tryClaim()
        if acquired && controlWaiters.load(ordering: .acquiring) > 0 {
            rendersWhileControlQueued.wrappingAdd(1, ordering: .relaxed)
        }
        return acquired
    }

    private func acquireControl(afterQueuing: (() -> Void)?) {
        controlWaiters.wrappingAdd(1, ordering: .acquiringAndReleasing)
        afterQueuing?()
        while !tryClaim() {
            Thread.sleep(forTimeInterval: 0)
        }
        rendersWhileControlQueued.store(0, ordering: .relaxed)
        controlWaiters.wrappingSubtract(1, ordering: .acquiringAndReleasing)
    }

    func acquireControl() { acquireControl(afterQueuing: nil) }

    #if SOTF_NATIVE_SMOKE
    func acquireControlForTesting(afterQueuing: @escaping () -> Void) {
        acquireControl(afterQueuing: afterQueuing)
    }
    #endif

    @inline(__always)
    func tryAcquireControl() -> Bool { tryClaim() }

    @inline(__always)
    func release() {
        claimed.store(false, ordering: .releasing)
    }

    func withControlAccess<T>(_ body: () throws -> T) rethrows -> T {
        acquireControl()
        defer { release() }
        return try body()
    }
}

private final class RenderStateOwner {
    let pointer: UnsafeMutablePointer<RenderState>
    private let accessGate: RenderAccessGate
    /// Owns the storage backing `RenderState.inputBufferList`.  The render
    /// block retains this owner, so the ABL remains valid even if a host keeps
    /// the block after releasing the AUAudioUnit object.
    var inputPullBuffer: AVAudioPCMBuffer?

    init(accessGate: RenderAccessGate) {
        self.accessGate = accessGate
        pointer = .allocate(capacity: 1)
        pointer.initialize(to: RenderState(
            handle: nil,
            inputChannels: 0,
            outputChannels: 0,
            inputBufferList: nil,
            scratchIn: nil,
            scratchOut: nil,
            ownedOutput: nil,
            scratchCapacity: 0,
            maximumFrames: 0,
            midiIn: nil,
            midiOut: nil,
            noteExpressionOut: nil,
            eventCapacity: 0,
            parameterIds: nil,
            parameterMetadata: nil,
            parameterRamps: nil,
            parameterMailboxes: nil,
            parameterCount: 0,
            parameterEpoch: 1,
            resourcesAllocated: false
        ))
    }

    deinit {
        accessGate.acquireControl()
        defer { accessGate.release() }
        let state = pointer.pointee
        if let handle = state.handle {
            plugin_destroy(handle)
        }
        state.scratchIn?.deallocate()
        state.scratchOut?.deallocate()
        state.ownedOutput?.deallocate()
        state.midiIn?.deallocate()
        state.midiOut?.deallocate()
        state.noteExpressionOut?.deallocate()
        if let parameterIds = state.parameterIds {
            for index in 0..<state.parameterCount {
                UnsafeMutablePointer(mutating: parameterIds[index])?.deallocate()
            }
            parameterIds.deallocate()
        }
        state.parameterMetadata?.deallocate()
        state.parameterRamps?.deallocate()
        if let mailboxes = state.parameterMailboxes {
            for index in 0..<state.parameterCount {
                mailboxes[index].release()
            }
            mailboxes.deallocate()
        }
        pointer.deinitialize(count: 1)
        pointer.deallocate()
    }
}

/// Base AUAudioUnit that delegates all processing to a Rust plugin via FFI.
///
/// Subclasses must override:
/// - `pluginType` (e.g., "Compressor")
/// - `pluginSubtype` (e.g., "SOCP", 4-char code)
/// - `pluginName` (e.g., "SOTF: Compressor")
open class GenericRustAudioUnit: AUAudioUnit {

    /// A hostile host must not be able to turn a negotiated block size into a
    /// trapping Swift allocation. This is far above normal realtime blocks
    /// while keeping the two scratch planes and owned output bounded.
    private static let maximumSupportedFrames: UInt32 = 65_536
    private static let maximumScratchSamples = 16_777_216

    private static func supportedMaximumChannelCount(isInput: Bool) -> AUAudioChannelCount {
        if let capabilities = supportedChannelCapabilities {
            let offset = isInput ? 0 : 1
            let widths = stride(from: offset, to: capabilities.count, by: 2).compactMap { index -> Int? in
                let value = capabilities[index].intValue
                if value > 0 { return value }
                if value < -2 { return abs(value) }
                return nil
            }
            if let maximum = widths.max() {
                return AVAudioChannelCount(maximum)
            }
        }
        if let fixed = isInput ? fixedInputChannels : fixedOutputChannels {
            return AVAudioChannelCount(fixed)
        }
        return 64
    }

    private static func isSupportedPCMFormat(_ format: AVAudioFormat) -> Bool {
        let sampleRate = format.sampleRate
        guard sampleRate.isFinite,
              sampleRate > 0,
              sampleRate <= Double(UInt32.max),
              sampleRate.rounded() == sampleRate,
              format.channelCount > 0,
              format.commonFormat == .pcmFormatFloat32,
              supportsSampleRate(sampleRate) else {
            return false
        }
        let asbd = format.streamDescription.pointee
        let requiredFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked
        let bytesPerFrame = format.isInterleaved
            ? UInt32(MemoryLayout<Float>.size) * format.channelCount
            : UInt32(MemoryLayout<Float>.size)
        return asbd.mFormatID == kAudioFormatLinearPCM
            && asbd.mBitsPerChannel == 32
            && asbd.mFramesPerPacket == 1
            && asbd.mBytesPerFrame == bytesPerFrame
            && asbd.mBytesPerPacket == bytesPerFrame
            && asbd.mFormatFlags & requiredFlags == requiredFlags
            && asbd.mFormatFlags & kAudioFormatFlagIsBigEndian == 0
    }

    private static func capabilityAllows(width: Int, isInput: Bool) -> Bool {
        if let fixed = isInput ? fixedInputChannels : fixedOutputChannels {
            return width == fixed
        }
        guard let capabilities = supportedChannelCapabilities else {
            return width <= Int(supportedMaximumChannelCount(isInput: isInput))
        }
        let offset = isInput ? 0 : 1
        return stride(from: offset, to: capabilities.count, by: 2).contains { index in
            let advertised = capabilities[index].intValue
            if advertised == -1 || advertised == -2 { return true }
            if advertised < -2 { return width > 0 && width <= abs(advertised) }
            return width == advertised
        }
    }

    private static func capabilityAllows(inputChannels: Int, outputChannels: Int) -> Bool {
        guard let capabilities = supportedChannelCapabilities else {
            if fixedInputChannels == nil && fixedOutputChannels == nil {
                return inputChannels == outputChannels
            }
            return capabilityAllows(width: inputChannels, isInput: true)
                && capabilityAllows(width: outputChannels, isInput: false)
        }
        for index in stride(from: 0, to: capabilities.count, by: 2) {
            guard index + 1 < capabilities.count else { return false }
            let input = capabilities[index].intValue
            let output = capabilities[index + 1].intValue
            // AUChannelInfo defines (-1, -1) as arbitrary but equal widths and
            // (-1, -2) as independently arbitrary widths. Values below -2 are
            // inclusive caps across the corresponding scope.
            if input == -1 && output == -1 {
                if inputChannels == outputChannels { return true }
                continue
            }
            if (input == -1 && output == -2) || (input == -2 && output == -1) {
                if inputChannels > 0 && outputChannels > 0 { return true }
                continue
            }
            let inputMatches = input == -1 || input == -2
                || (input < -2 && inputChannels <= abs(input))
                || input == inputChannels
            let outputMatches = output == -1 || output == -2
                || (output < -2 && outputChannels <= abs(output))
                || output == outputChannels
            if inputMatches && outputMatches {
                return true
            }
        }
        return false
    }

    #if SOTF_NATIVE_SMOKE
    class func channelPairAllowedForTesting(
        inputChannels: Int,
        outputChannels: Int
    ) -> Bool {
        capabilityAllows(inputChannels: inputChannels, outputChannels: outputChannels)
    }
    #endif

    private static func provisionalFormat(channels: AVAudioChannelCount) -> AVAudioFormat? {
        if channels <= 2 {
            return AVAudioFormat(standardFormatWithSampleRate: 48000, channels: channels)
        }
        let tag = kAudioChannelLayoutTag_DiscreteInOrder | AudioChannelLayoutTag(channels)
        guard let layout = AVAudioChannelLayout(layoutTag: tag) else { return nil }
        return AVAudioFormat(standardFormatWithSampleRate: 48000, channelLayout: layout)
    }

    // MARK: - Subclass Configuration (override these)

    /// Rust plugin type name passed to plugin_create()
    open class var pluginType: String { fatalError("Subclass must override pluginType") }

    /// AU subtype (4-char code)
    open class var pluginSubtype: String { fatalError("Subclass must override pluginSubtype") }

    /// Display name shown in DAW
    open class var pluginName: String { "SOTF Plugin" }

    /// Fixed output width for channel-changing effects. Nil means input and
    /// output widths must match.
    open class var fixedOutputChannels: Int? { nil }

    /// Fixed input width for effects whose DSP topology is not host-variable.
    /// Nil means the plugin may negotiate any supported input width.
    open class var fixedInputChannels: Int? { nil }

    /// Exact input/output capability pairs for plugins that support a discrete
    /// set of layouts rather than every width. Values use the Core Audio
    /// AUChannelInfo flattened representation.
    open class var supportedChannelCapabilities: [NSNumber]? { nil }

    /// Provisional bus widths used before the host negotiates its render
    /// formats. Channel-changing subclasses override these so construction can
    /// create a valid parameter-bearing Rust instance.
    open class var initialInputChannels: AVAudioChannelCount { 2 }
    open class var initialOutputChannels: AVAudioChannelCount {
        fixedOutputChannels.map(AVAudioChannelCount.init) ?? initialInputChannels
    }

    /// Sample-rate predicate used consistently by bus negotiation and final
    /// resource validation. Most plugins accept every positive integral rate.
    open class func supportsSampleRate(_ sampleRate: Double) -> Bool { true }

    /// Serialized construction state derived from the negotiated bus formats.
    open func pluginConfiguration(inputFormat: AVAudioFormat,
                                  outputFormat: AVAudioFormat) -> String { "{}" }

    // MARK: - Properties

    private var inputBus: AUAudioUnitBus
    private var outputBus: AUAudioUnitBus
    private var _inputBusArray: AUAudioUnitBusArray!
    private var _outputBusArray: AUAudioUnitBusArray!
    private var _parameterTree: AUParameterTree?
    private var auParameters: [AUParameter] = []
    private var _maxFramesToRender: UInt32 = 4096
    /// Host queries may arrive independently of resource negotiation. Publish
    /// the control-thread latency calculation as immutable bits so the getter
    /// never crosses the FFI boundary or allocates.
    private let latencySecondsBits = Atomic<UInt64>(0)

    /// Current Rust plugin configuration — used to detect when re-creation is needed
    private var rustSampleRate: UInt32 = 0
    private var rustConfiguration: String?
    private var rustMaximumFramesToRender: UInt32 = 0
    #if SOTF_NATIVE_SMOKE
    private var migrationStateOverrideForTesting: Data?
    private var allocationFailureStageForTesting = 0

    func injectMigrationStateForTesting(_ data: Data) {
        migrationStateOverrideForTesting = data
    }

    func injectAllocationFailureForTesting(stage: Int) {
        allocationFailureStageForTesting = stage
    }

    func holdRenderAccessForTesting(_ body: () -> Void) {
        accessGate.acquireControl()
        defer { accessGate.release() }
        body()
    }

    @discardableResult
    func enqueueParameterForTesting(address: UInt64, normalizedValue: Float) -> Bool {
        // Native-smoke-only raw producer hook. Tests keep the schema fixed
        // while using it, so the retained mailbox table cannot be replaced.
        guard address < UInt64(renderStatePtr.pointee.parameterCount),
              let mailboxes = renderStatePtr.pointee.parameterMailboxes else { return false }
        mailboxes[Int(address)].takeUnretainedValue().publish(normalizedValue)
        return true
    }

    @discardableResult
    func enqueueStaleParameterForTesting(address: UInt64, normalizedValue: Float) -> Bool {
        false
    }

    func parameterValueForTesting(identifier: String) -> AUValue? {
        guard let parameter = auParameters.first(where: { $0.identifier == identifier }) else {
            return nil
        }
        return readParameterFromRust(param: parameter)
    }

    func parameterQueueStateForTesting() -> (count: Int, publishedEpoch: UInt64, renderEpoch: UInt64) {
        accessGate.withControlAccess {
            var pending = 0
            if let mailboxes = renderStatePtr.pointee.parameterMailboxes {
                for index in 0..<renderStatePtr.pointee.parameterCount {
                    if mailboxes[index].takeUnretainedValue().pendingValue() != nil { pending += 1 }
                }
            }
            return (
                pending,
                parameterEpoch.load(ordering: .acquiring),
                renderStatePtr.pointee.parameterEpoch
            )
        }
    }

    func retainsRenderStorageForTesting() -> Bool {
        accessGate.withControlAccess {
            renderStatePtr.pointee.scratchIn != nil
                || renderStatePtr.pointee.scratchOut != nil
                || renderStatePtr.pointee.ownedOutput != nil
                || renderStatePtr.pointee.midiIn != nil
                || renderStatePtr.pointee.midiOut != nil
                || renderStatePtr.pointee.noteExpressionOut != nil
                || renderStatePtr.pointee.inputBufferList != nil
                || renderStateOwner.inputPullBuffer != nil
        }
    }

    func parameterStepsForTesting(address: UInt64) -> UInt32? {
        accessGate.withControlAccess {
            guard address < UInt64(renderStatePtr.pointee.parameterCount),
                  let metadata = renderStatePtr.pointee.parameterMetadata else { return nil }
            return metadata[Int(address)].steps
        }
    }

    func parameterIsRealtimeForTesting(address: UInt64) -> Bool? {
        accessGate.withControlAccess {
            guard address < UInt64(renderStatePtr.pointee.parameterCount),
                  let metadata = renderStatePtr.pointee.parameterMetadata else { return nil }
            return metadata[Int(address)].realtime
        }
    }

    func rebuildParameterTreeForTesting() {
        accessGate.withControlAccess { buildParameterTreeLocked() }
    }

    func renderBurstBeforeQueuedControlWinsForTesting() -> Int {
        guard accessGate.tryAcquireRender() else { return -1 }
        let queued = DispatchSemaphore(value: 0)
        let acquired = DispatchSemaphore(value: 0)
        Thread.detachNewThread { [accessGate] in
            accessGate.acquireControlForTesting { queued.signal() }
            acquired.signal()
            accessGate.release()
        }
        guard queued.wait(timeout: .now() + 5) == .success else {
            accessGate.release()
            return -1
        }
        accessGate.release()
        var renderClaims = 0
        while acquired.wait(timeout: .now()) != .success {
            if accessGate.tryAcquireRender() {
                renderClaims += 1
                accessGate.release()
            }
        }
        return renderClaims
    }
    #endif

    /// A preallocated single-owner gate. The render callback never waits: it
    /// either acquires the Rust handle for the whole block or asks the host to
    /// retry. Control/UI operations may wait, but all allocation, state
    /// serialization, migration, and destruction therefore stay off the
    /// real-time thread.
    private let accessGate: RenderAccessGate
    /// Changes whenever the Rust instance or its host-visible parameter schema
    /// is replaced. Commands are stamped at publication so a command for a
    /// retired address space can never be applied to the replacement plugin.
    private let parameterEpoch = Atomic<UInt64>(1)

    /// Heap-allocated render state shared with the render block via pointer.
    /// The render block captures `renderStatePtr` once; allocateRenderResources
    /// updates the pointed-to struct so the render thread always sees current values.
    private let renderStateOwner: RenderStateOwner
    private var renderStatePtr: UnsafeMutablePointer<RenderState> {
        renderStateOwner.pointer
    }

    /// Test/diagnostic visibility without exposing a mutable Rust pointer to
    /// arbitrary UI threads.
    public var hasRustPlugin: Bool {
        accessGate.withControlAccess { renderStatePtr.pointee.handle != nil }
    }

    public var coalescedParameterPublicationCount: UInt64 {
        accessGate.withControlAccess {
            guard let mailboxes = renderStatePtr.pointee.parameterMailboxes else { return 0 }
            var total: UInt64 = 0
            for index in 0..<renderStatePtr.pointee.parameterCount {
                total &+= mailboxes[index].takeUnretainedValue().coalescedCount.load(ordering: .relaxed)
            }
            return total
        }
    }

    // MARK: - Initialization

    public override init(componentDescription: AudioComponentDescription,
                        options: AudioComponentInstantiationOptions = []) throws {
        let gate = RenderAccessGate()
        accessGate = gate
        renderStateOwner = RenderStateOwner(accessGate: gate)

        // AUAudioUnit requires the stored buses before super.init. Construct
        // them at the subclass's truthful provisional topology so fixed
        // channel pairs never pass through an invalid intermediate state.
        guard let defaultInputFormat = Self.provisionalFormat(
            channels: Self.initialInputChannels
        ), let defaultOutputFormat = Self.provisionalFormat(
            channels: Self.initialOutputChannels
        ) else {
            throw NSError(domain: NSOSStatusErrorDomain, code: Int(kAudioUnitErr_FormatNotSupported))
        }

        inputBus = try AUAudioUnitBus(format: defaultInputFormat)
        outputBus = try AUAudioUnitBus(format: defaultOutputFormat)

        inputBus.maximumChannelCount = Self.supportedMaximumChannelCount(isInput: true)
        outputBus.maximumChannelCount = Self.supportedMaximumChannelCount(isInput: false)

        try super.init(componentDescription: componentDescription, options: options)

        _inputBusArray = AUAudioUnitBusArray(audioUnit: self, busType: .input, busses: [inputBus])
        _outputBusArray = AUAudioUnitBusArray(audioUnit: self, busType: .output, busses: [outputBus])

        // Create initial Rust plugin with default format
        let sampleRate = UInt32(defaultInputFormat.sampleRate)
        try createRustPlugin(inputFormat: defaultInputFormat, outputFormat: defaultOutputFormat,
                             sampleRate: sampleRate)
        buildParameterTree()
    }

    // MARK: - Rust Plugin Lifecycle

    /// Apply the newest coalesced value for every address. The caller owns the
    /// Rust handle through `accessGate`; producers only touch mailbox atomics.
    @inline(__always)
    private func drainParameterMailboxesLocked(handle: OpaquePointer) -> Bool {
        guard let ids = renderStatePtr.pointee.parameterIds,
              let mailboxes = renderStatePtr.pointee.parameterMailboxes else { return true }
        var accepted = true
        for address in 0..<renderStatePtr.pointee.parameterCount {
            let mailbox = mailboxes[address].takeUnretainedValue()
            guard let pending = mailbox.pendingValue() else { continue }
            // Retire even a rejected publication so one malformed producer
            // cannot poison every subsequent render call.
            mailbox.markApplied(pending.version)
            guard pending.value.isFinite,
                  (0.0...1.0).contains(pending.value),
                  let id = ids[address],
                  plugin_set_parameter(handle, id, Double(pending.value)) == 0 else {
                accepted = false
                continue
            }
        }
        return accepted
    }

    private func createRustPlugin(inputFormat: AVAudioFormat,
                                  outputFormat: AVAudioFormat,
                                  sampleRate: UInt32) throws {
        willChangeValue(forKey: "latency")
        defer { didChangeValue(forKey: "latency") }
        try accessGate.withControlAccess {
            try createRustPluginLocked(
                inputFormat: inputFormat,
                outputFormat: outputFormat,
                sampleRate: sampleRate
            )
        }
    }

    /// `plugin_get_info_json` owns its returned string and may allocate, so
    /// latency is decoded only while the control gate is held. The AU latency
    /// getter reads the published scalar and remains realtime-safe.
    private func latencySecondsLocked(handle: OpaquePointer, sampleRate: UInt32) -> TimeInterval? {
        guard sampleRate > 0, let info = plugin_get_info_json(handle) else { return nil }
        defer { plugin_free_string(info) }
        guard let data = String(cString: info).data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data),
              let dictionary = object as? [String: Any],
              let latencySamples = dictionary["latency_samples"] as? NSNumber else {
            return nil
        }
        return latencySamples.doubleValue / Double(sampleRate)
    }

    /// Add facade-only construction metadata without mutating the plugin's
    /// persisted/base configuration. Rust ignores the reserved field after
    /// using it to size direct-format realtime adapters.
    private func ffiConfiguration(_ configuration: String) -> String? {
        guard let data = configuration.data(using: .utf8),
              var object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return nil
        }
        object["_sotf_max_callback_frames"] = NSNumber(value: maximumFramesToRender)
        guard let encoded = try? JSONSerialization.data(withJSONObject: object) else {
            return nil
        }
        return String(data: encoded, encoding: .utf8)
    }

    @inline(__always)
    private func publishLatency(_ seconds: TimeInterval) {
        latencySecondsBits.store(seconds.bitPattern, ordering: .releasing)
    }

    /// Caller owns `accessGate` for the complete snapshot/create/migrate/swap
    /// transaction.
    private func createRustPluginLocked(inputFormat: AVAudioFormat,
                                        outputFormat: AVAudioFormat,
                                        sampleRate: UInt32) throws {
        let pluginType = type(of: self).pluginType
        let inputChannels = Int(inputFormat.channelCount)
        let outputChannels = Int(outputFormat.channelCount)
        let configuration = pluginConfiguration(inputFormat: inputFormat,
                                                outputFormat: outputFormat)

        // Capture compatible state before creating the replacement. Keep the
        // old handle alive until the candidate is fully initialized so a
        // rejected format never strands the AU without its last valid state.
        let oldHandle = renderStatePtr.pointee.handle
        let topologyUnchanged = renderStatePtr.pointee.inputChannels == inputChannels
            && renderStatePtr.pointee.outputChannels == outputChannels
            && rustConfiguration == configuration
        var savedState: Data?
        var savedParameters: [(String, Double)] = []
        if let oldHandle {
            guard drainParameterMailboxesLocked(handle: oldHandle) else {
                throw NSError(
                    domain: NSOSStatusErrorDomain,
                    code: Int(kAudioUnitErr_InvalidParameter)
                )
            }
            // Whole-plugin state may contain construction- or topology-sized
            // values (for example ChannelMuteSolo.channel_states). It is only
            // portable when the channel topology is unchanged. Across a
            // topology change migrate the intersection of host-visible scalar
            // parameters instead.
            if topologyUnchanged {
                var stateLength = 0
                if let state = plugin_save_state(oldHandle, &stateLength), stateLength > 0 {
                    savedState = Data(bytes: state, count: stateLength)
                    plugin_free_state(state, stateLength)
                }
            }
            savedParameters = auParameters.compactMap { parameter in
                let value = parameter.identifier.withCString { identifier in
                    plugin_get_parameter(oldHandle, identifier)
                }
                return value >= 0 ? (parameter.identifier, value) : nil
            }
        }
        #if SOTF_NATIVE_SMOKE
        if let override = migrationStateOverrideForTesting {
            savedState = override
            migrationStateOverrideForTesting = nil
        }
        #endif

        guard let ffiConfiguration = ffiConfiguration(configuration) else {
            throw NSError(
                domain: NSOSStatusErrorDomain,
                code: Int(kAudioUnitErr_InvalidPropertyValue),
                userInfo: [NSLocalizedDescriptionKey: "Invalid Rust plugin construction JSON"]
            )
        }

        let candidate = pluginType.withCString { typePtr in
            ffiConfiguration.withCString { configPtr in
                plugin_create(
                    typePtr,
                    configPtr,
                    sampleRate,
                    inputChannels,
                    outputChannels
                )
            }
        }

        guard let candidate else {
            let error = plugin_get_last_error()
            let msg = error != nil ? String(cString: error!) : "Unknown error"
            NSLog("SOTF: Failed to create \(pluginType) plugin (\(inputChannels)→\(outputChannels)ch, \(sampleRate)Hz): \(msg)")
            throw NSError(
                domain: NSOSStatusErrorDomain,
                code: Int(kAudioUnitErr_FailedInitialization),
                userInfo: [NSLocalizedDescriptionKey: msg]
            )
        }
        var candidateCommitted = false
        defer {
            if !candidateCommitted {
                plugin_destroy(candidate)
            }
        }
        guard plugin_reset(candidate) == 0 else {
            throw NSError(
                domain: NSOSStatusErrorDomain,
                code: Int(kAudioUnitErr_FailedInitialization),
                userInfo: [NSLocalizedDescriptionKey: "Rust plugin reset failed"]
            )
        }

        if let savedState {
            let status = savedState.withUnsafeBytes { bytes -> Int32 in
                guard let base = bytes.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                    return -1
                }
                return plugin_load_state(candidate, base, bytes.count)
            }
            guard status == 0 else {
                throw NSError(
                    domain: NSOSStatusErrorDomain,
                    code: Int(kAudioUnitErr_FailedInitialization),
                    userInfo: [NSLocalizedDescriptionKey: "Rust plugin state migration failed"]
                )
            }
        }
        // State serialization preserves plugin-owned state, while the FFI
        // parameter map is the authoritative host-visible automation state.
        // Reapply that intersection even after a successful state load.
        for (identifier, value) in savedParameters {
                var candidateHasParameter = false
                let parameterCount = plugin_get_parameter_count(candidate)
                if parameterCount > 0 {
                    for index in 0..<Int(parameterCount) {
                        guard let info = plugin_get_parameter_info(candidate, index) else { continue }
                        if String(cString: info.pointee.id) == identifier {
                            candidateHasParameter = true
                            break
                        }
                    }
                }
                // Topology-specific parameters that disappeared are not
                // portable. New parameters retain their candidate defaults.
                guard candidateHasParameter else { continue }
                let candidateValue = identifier.withCString { id in
                    plugin_get_parameter(candidate, id)
                }
                if candidateValue >= 0, abs(candidateValue - value) <= 1.0e-12 {
                    continue
                }
                let status = identifier.withCString { id in
                    plugin_set_parameter(candidate, id, value)
                }
                if status != 0 {
                    // A shared identifier can still have topology-dependent
                    // validity or representation. Preserve the candidate's
                    // valid default rather than rejecting an otherwise valid
                    // format change.
                    continue
                }
        }

        guard let candidateLatency = latencySecondsLocked(handle: candidate, sampleRate: sampleRate)
        else {
            throw NSError(
                domain: NSOSStatusErrorDomain,
                code: Int(kAudioUnitErr_FailedInitialization),
                userInfo: [NSLocalizedDescriptionKey: "Rust plugin latency metadata is unavailable"]
            )
        }

        if let oldHandle {
            plugin_destroy(oldHandle)
        }
        renderStatePtr.pointee.handle = candidate
        renderStatePtr.pointee.inputChannels = inputChannels
        renderStatePtr.pointee.outputChannels = outputChannels
        rustSampleRate = sampleRate
        rustConfiguration = configuration
        rustMaximumFramesToRender = maximumFramesToRender
        publishLatency(candidateLatency)
        candidateCommitted = true
    }

    /// Apply serialized state to a fresh candidate and publish it only after
    /// the complete load succeeds. The live handle is therefore unchanged by
    /// malformed or partially invalid state documents.
    private func replacePluginStateLocked(
        _ data: Data,
        parameterValues: [(identifier: String, normalized: Double)] = []
    ) -> Bool {
        guard let configuration = rustConfiguration,
              let oldHandle = renderStatePtr.pointee.handle else {
            return false
        }
        guard let ffiConfiguration = ffiConfiguration(configuration) else { return false }
        let candidate = type(of: self).pluginType.withCString { typePtr in
            ffiConfiguration.withCString { configPtr in
                plugin_create(
                    typePtr,
                    configPtr,
                    rustSampleRate,
                    renderStatePtr.pointee.inputChannels,
                    renderStatePtr.pointee.outputChannels
                )
            }
        }
        guard let candidate else { return false }
        var committed = false
        defer {
            if !committed { plugin_destroy(candidate) }
        }
        guard plugin_reset(candidate) == 0 else { return false }
        let status = data.withUnsafeBytes { bytes -> Int32 in
            guard let ptr = bytes.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                return -1
            }
            return plugin_load_state(candidate, ptr, bytes.count)
        }
        guard status == 0 else { return false }
        // `plugin_save_state` covers plugin-owned state, while the FFI
        // parameter map is the host-visible automation contract. Restore both
        // on the isolated candidate before publishing it.
        for value in parameterValues {
            guard value.normalized.isFinite,
                  (0.0...1.0).contains(value.normalized),
                  value.identifier.withCString({ id in
                      plugin_set_parameter(candidate, id, value.normalized)
                  }) == 0 else {
                return false
            }
        }
        guard let candidateLatency = latencySecondsLocked(
            handle: candidate,
            sampleRate: rustSampleRate
        ) else { return false }
        renderStatePtr.pointee.handle = candidate
        plugin_destroy(oldHandle)
        rustMaximumFramesToRender = maximumFramesToRender
        publishLatency(candidateLatency)
        advanceParameterEpochLocked()
        committed = true
        return true
    }

    /// Import a document into an isolated candidate so malformed or partially
    /// applicable state can never mutate the live Rust instance.
    private func replacePluginPresetDocumentLocked(_ data: Data) -> Bool {
        guard let configuration = rustConfiguration,
              let oldHandle = renderStatePtr.pointee.handle else { return false }
        guard let ffiConfiguration = ffiConfiguration(configuration) else { return false }
        let candidate = type(of: self).pluginType.withCString { typePtr in
            ffiConfiguration.withCString { configPtr in
                plugin_create(
                    typePtr,
                    configPtr,
                    rustSampleRate,
                    renderStatePtr.pointee.inputChannels,
                    renderStatePtr.pointee.outputChannels
                )
            }
        }
        guard let candidate else { return false }
        var committed = false
        defer { if !committed { plugin_destroy(candidate) } }
        guard plugin_reset(candidate) == 0 else { return false }
        let imported = data.withUnsafeBytes { bytes -> Bool in
            guard let pointer = bytes.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                return false
            }
            return plugin_import_preset_json(candidate, pointer, bytes.count) == 0
        }
        guard imported else { return false }
        guard let candidateLatency = latencySecondsLocked(
            handle: candidate,
            sampleRate: rustSampleRate
        ) else { return false }
        renderStatePtr.pointee.handle = candidate
        plugin_destroy(oldHandle)
        rustMaximumFramesToRender = maximumFramesToRender
        publishLatency(candidateLatency)
        if parameterSchemaMatchesLocked() {
            advanceParameterEpochLocked()
        } else {
            buildParameterTreeLocked()
        }
        committed = true
        return true
    }

    // MARK: - Parameter Tree

    private func buildParameterTree() {
        accessGate.acquireControl()
        defer { accessGate.release() }
        buildParameterTreeLocked()
    }

    /// Rebuilds every host-visible parameter artifact as one handle-owned
    /// transaction. The caller holds `accessGate`; the epoch is published only
    /// after the tree, identifiers, metadata, and ramp storage all agree.
    private func buildParameterTreeLocked() {
        guard let handle = renderStatePtr.pointee.handle else { return }

        let paramCount = plugin_get_parameter_count(handle)
        let count = max(Int(paramCount), 0)
        var params: [AUParameter] = []
        var metadata: [CachedParameterMetadata] = []

        for i in 0..<count {
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

            var parameterFlags: AudioUnitParameterOptions = [.flag_IsReadable, .flag_IsWritable]
            // The FFI ParameterInfo contract exposes automation granularity via
            // `steps`; continuous parameters are the ones that can be ramped.
            let realtime = info.pointee.steps == 0
            if realtime {
                parameterFlags.insert(.flag_CanRamp)
            }
            let param = AUParameterTree.createParameter(
                withIdentifier: paramId,
                name: paramName,
                address: AUParameterAddress(i),
                min: AUValue(info.pointee.min_value),
                max: AUValue(info.pointee.max_value),
                unit: auUnit,
                unitName: unitStr.isEmpty ? nil : unitStr,
                flags: parameterFlags,
                valueStrings: nil,
                dependentParameters: nil
            )
            param.value = AUValue(info.pointee.default_value)
            params.append(param)
            metadata.append(CachedParameterMetadata(
                minValue: info.pointee.min_value,
                maxValue: info.pointee.max_value,
                logarithmic: info.pointee.logarithmic,
                steps: info.pointee.steps,
                realtime: realtime
            ))
        }

        let mailboxObjects = params.map { _ in ParameterMailbox() }
        let newMailboxes = UnsafeMutablePointer<Unmanaged<ParameterMailbox>>.allocate(
            capacity: max(params.count, 1)
        )
        for (index, mailbox) in mailboxObjects.enumerated() {
            newMailboxes[index] = Unmanaged.passRetained(mailbox)
        }

        if let oldIds = renderStatePtr.pointee.parameterIds {
            for index in 0..<renderStatePtr.pointee.parameterCount {
                UnsafeMutablePointer(mutating: oldIds[index])?.deallocate()
            }
            oldIds.deallocate()
        }
        renderStatePtr.pointee.parameterMetadata?.deallocate()
        renderStatePtr.pointee.parameterRamps?.deallocate()
        if let oldMailboxes = renderStatePtr.pointee.parameterMailboxes {
            for index in 0..<renderStatePtr.pointee.parameterCount {
                oldMailboxes[index].release()
            }
            oldMailboxes.deallocate()
        }
        let ids = UnsafeMutablePointer<UnsafePointer<CChar>?>.allocate(
            capacity: max(params.count, 1)
        )
        let cachedMetadata = UnsafeMutablePointer<CachedParameterMetadata>.allocate(
            capacity: max(params.count, 1)
        )
        let ramps = UnsafeMutablePointer<ParameterRampState>.allocate(
            capacity: max(params.count, 1)
        )
        for (index, parameter) in params.enumerated() {
            let utf8 = Array(parameter.identifier.utf8CString)
            let stored = UnsafeMutablePointer<CChar>.allocate(capacity: utf8.count)
            stored.initialize(from: utf8, count: utf8.count)
            ids[index] = UnsafePointer(stored)
            cachedMetadata[index] = metadata[index]
            ramps[index] = ParameterRampState()
        }
        renderStatePtr.pointee.parameterIds = ids
        renderStatePtr.pointee.parameterMetadata = cachedMetadata
        renderStatePtr.pointee.parameterRamps = ramps
        renderStatePtr.pointee.parameterMailboxes = newMailboxes
        renderStatePtr.pointee.parameterCount = params.count
        auParameters = params
        _parameterTree = AUParameterTree.createTree(withChildren: params)

        _parameterTree?.implementorValueObserver = { [weak self, mailboxObjects] param, value in
            guard param.address < UInt64(mailboxObjects.count) else { return }
            self?.syncParameterToRust(
                param: param,
                value: value,
                mailbox: mailboxObjects[Int(param.address)]
            )
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
        advanceParameterEpochLocked(resetRamps: false)
    }

    private func parameterSchemaMatchesLocked() -> Bool {
        guard let handle = renderStatePtr.pointee.handle,
              let ids = renderStatePtr.pointee.parameterIds,
              let metadata = renderStatePtr.pointee.parameterMetadata else {
            return false
        }
        let count = max(Int(plugin_get_parameter_count(handle)), 0)
        guard count == renderStatePtr.pointee.parameterCount else { return false }
        for index in 0..<count {
            guard let info = plugin_get_parameter_info(handle, index),
                  let id = ids[index],
                  strcmp(info.pointee.id, id) == 0,
                  info.pointee.min_value == metadata[index].minValue,
                  info.pointee.max_value == metadata[index].maxValue,
                  info.pointee.logarithmic == metadata[index].logarithmic,
                  info.pointee.steps == metadata[index].steps else {
                return false
            }
        }
        return true
    }

    private func advanceParameterEpochLocked(resetRamps: Bool = true) {
        if resetRamps, let ramps = renderStatePtr.pointee.parameterRamps {
            for index in 0..<renderStatePtr.pointee.parameterCount {
                ramps[index] = ParameterRampState()
            }
        }
        if let mailboxes = renderStatePtr.pointee.parameterMailboxes {
            for index in 0..<renderStatePtr.pointee.parameterCount {
                mailboxes[index].takeUnretainedValue().discardPending()
            }
        }
        let nextEpoch = parameterEpoch.load(ordering: .relaxed) &+ 1
        renderStatePtr.pointee.parameterEpoch = nextEpoch
        parameterEpoch.store(nextEpoch, ordering: .releasing)
    }

    private func syncParameterToRust(
        param: AUParameter,
        value: AUValue,
        mailbox: ParameterMailbox? = nil
    ) {
        // AU value observers may be called by an audio-thread automation
        // producer. Never wait for the render owner here; publish to a bounded,
        // preallocated queue and let the render owner apply it at the next
        // block boundary. Repeated writes coalesce in the mailbox and are
        // observable through `coalescedParameterPublicationCount`.
        let normalized = normalize(value: value, param: param)
        guard normalized.isFinite, (0.0...1.0).contains(normalized) else { return }
        if let mailbox {
            mailbox.publish(normalized)
            return
        }
        // Compatibility path for callers which did not originate from the
        // current tree observer. Resolve under control ownership; stale trees
        // never use this path because their observer captures its generation.
        accessGate.withControlAccess {
            guard param.address < UInt64(renderStatePtr.pointee.parameterCount),
                  let mailboxes = renderStatePtr.pointee.parameterMailboxes else { return }
            mailboxes[Int(param.address)].takeUnretainedValue().publish(normalized)
        }
    }

    private func readParameterFromRust(param: AUParameter) -> AUValue {
        // Never access param.value in this method — it triggers implementorValueProvider
        // which calls back into this method, causing infinite recursion.
        accessGate.withControlAccess {
            guard let handle = renderStatePtr.pointee.handle else { return param.minValue }
            guard drainParameterMailboxesLocked(handle: handle) else { return param.minValue }
            let paramId = param.identifier
            let normalized = paramId.withCString { idPtr in
                plugin_get_parameter(handle, idPtr)
            }
            if normalized < 0 { return param.minValue }
            return denormalize(normalized: Float(normalized), param: param)
        }
    }

    /// Returns parameter metadata while the Rust handle is exclusively owned;
    /// callers apply the value through AUParameterTree after the pointer is no
    /// longer borrowed.
    public func defaultValueForParameter(at index: Int) -> AUValue? {
        accessGate.withControlAccess {
            guard let handle = renderStatePtr.pointee.handle,
                  let info = plugin_get_parameter_info(handle, index) else {
                return nil
            }
            return AUValue(info.pointee.default_value)
        }
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
        get { accessGate.withControlAccess { _parameterTree } }
        set { accessGate.withControlAccess { _parameterTree = newValue } }
    }

    public override var inputBusses: AUAudioUnitBusArray { return _inputBusArray }
    public override var outputBusses: AUAudioUnitBusArray { return _outputBusArray }

    public override var maximumFramesToRender: AUAudioFrameCount {
        get { return _maxFramesToRender }
        set { _maxFramesToRender = newValue }
    }

    public override var latency: TimeInterval {
        Double(bitPattern: latencySecondsBits.load(ordering: .acquiring))
    }

    public override var channelCapabilities: [NSNumber]? {
        // During init the two provisional bus formats are assigned one at a
        // time. Advertising the final fixed pair before both bus arrays exist
        // makes Core Audio reject the intermediate half-updated topology.
        if _inputBusArray == nil || _outputBusArray == nil {
            return [NSNumber(value: -1), NSNumber(value: -1)]
        }
        if let exact = type(of: self).supportedChannelCapabilities {
            return exact
        }
        if type(of: self).fixedInputChannels != nil
            || type(of: self).fixedOutputChannels != nil {
            let inputs = type(of: self).fixedInputChannels ?? -1
            let outputs = type(of: self).fixedOutputChannels ?? -1
            return [NSNumber(value: inputs), NSNumber(value: outputs)]
        }
        return [NSNumber(value: -1), NSNumber(value: -1)]
    }

    public override func shouldChange(to format: AVAudioFormat, for bus: AUAudioUnitBus) -> Bool {
        guard Self.isSupportedPCMFormat(format) else { return false }
        if bus === inputBus {
            return Self.capabilityAllows(width: Int(format.channelCount), isInput: true)
        }
        if bus === outputBus {
            return Self.capabilityAllows(width: Int(format.channelCount), isInput: false)
        }
        return false
    }

    public override func allocateRenderResources() throws {
        try super.allocateRenderResources()
        // A recreation may replace static parameter metadata. Pair the KVO
        // transaction outside `accessGate` so observers can safely query the
        // newly published tree from `didChangeValue` without re-entering it.
        willChangeValue(forKey: "parameterTree")
        willChangeValue(forKey: "latency")
        var completed = false
        defer {
            didChangeValue(forKey: "latency")
            didChangeValue(forKey: "parameterTree")
            if !completed {
                super.deallocateRenderResources()
            }
        }

        // Construct a complete candidate bundle before acquiring the render
        // owner. No live pointer is changed until every fallible preparation
        // step has succeeded.
        let inputChannels = Int(inputBus.format.channelCount)
        let outputChannels = Int(outputBus.format.channelCount)
        guard Self.isSupportedPCMFormat(inputBus.format),
              Self.isSupportedPCMFormat(outputBus.format),
              inputBus.format.sampleRate == outputBus.format.sampleRate,
              Self.capabilityAllows(
                  inputChannels: inputChannels,
                  outputChannels: outputChannels
              ) else {
            throw NSError(domain: NSOSStatusErrorDomain, code: Int(kAudioUnitErr_FormatNotSupported))
        }
        let maxFrames = Int(maximumFramesToRender)
        let capacityResult = maxFrames.multipliedReportingOverflow(
            by: max(max(inputChannels, outputChannels), 1)
        )
        guard maxFrames > 0,
              maximumFramesToRender <= Self.maximumSupportedFrames,
              !capacityResult.overflow,
              capacityResult.partialValue <= Self.maximumScratchSamples else {
            throw NSError(domain: NSOSStatusErrorDomain, code: Int(kAudioUnitErr_TooManyFramesToProcess))
        }
        let needed = capacityResult.partialValue
        let candidateScratchIn = UnsafeMutablePointer<Float>.allocate(capacity: needed)
        let candidateScratchOut = UnsafeMutablePointer<Float>.allocate(capacity: needed)
        let candidateOwnedOutput = UnsafeMutablePointer<Float>.allocate(capacity: needed)
        var candidateCommitted = false
        let eventCapacity = 256
        var candidateMidiIn: UnsafeMutablePointer<PluginMidiEvent>?
        var candidateMidiOut: UnsafeMutablePointer<PluginMidiEvent>?
        var candidateNoteOut: UnsafeMutablePointer<PluginNoteExpressionEvent>?
        defer {
            if !candidateCommitted {
                candidateScratchIn.deallocate()
                candidateScratchOut.deallocate()
                candidateOwnedOutput.deallocate()
                candidateMidiIn?.deallocate()
                candidateMidiOut?.deallocate()
                candidateNoteOut?.deallocate()
            }
        }
        #if SOTF_NATIVE_SMOKE
        if allocationFailureStageForTesting == 1 {
            allocationFailureStageForTesting = 0
            throw NSError(domain: NSOSStatusErrorDomain, code: Int(kAudioUnitErr_FailedInitialization))
        }
        #endif
        guard let candidatePullBuffer = AVAudioPCMBuffer(
            pcmFormat: inputBus.format,
            frameCapacity: maximumFramesToRender
        ) else {
            throw NSError(domain: NSOSStatusErrorDomain, code: Int(kAudioUnitErr_FormatNotSupported))
        }
        #if SOTF_NATIVE_SMOKE
        if allocationFailureStageForTesting == 2 {
            allocationFailureStageForTesting = 0
            throw NSError(domain: NSOSStatusErrorDomain, code: Int(kAudioUnitErr_FailedInitialization))
        }
        #endif
        candidateMidiIn = .allocate(capacity: eventCapacity)
        candidateMidiOut = .allocate(capacity: eventCapacity)
        candidateNoteOut = .allocate(capacity: eventCapacity)
        #if SOTF_NATIVE_SMOKE
        if allocationFailureStageForTesting == 3 {
            allocationFailureStageForTesting = 0
            throw NSError(domain: NSOSStatusErrorDomain, code: Int(kAudioUnitErr_FailedInitialization))
        }
        #endif

        var oldScratchIn: UnsafeMutablePointer<Float>?
        var oldScratchOut: UnsafeMutablePointer<Float>?
        var oldOwnedOutput: UnsafeMutablePointer<Float>?
        var oldMidiIn: UnsafeMutablePointer<PluginMidiEvent>?
        var oldMidiOut: UnsafeMutablePointer<PluginMidiEvent>?
        var oldNoteOut: UnsafeMutablePointer<PluginNoteExpressionEvent>?
        var oldPullBuffer: AVAudioPCMBuffer?
        try accessGate.withControlAccess {
            #if SOTF_NATIVE_SMOKE
            if allocationFailureStageForTesting == 4 {
                allocationFailureStageForTesting = 0
                throw NSError(domain: NSOSStatusErrorDomain, code: Int(kAudioUnitErr_FailedInitialization))
            }
            #endif
            let sampleRate = UInt32(inputBus.format.sampleRate)
            let configuration = pluginConfiguration(
                inputFormat: inputBus.format,
                outputFormat: outputBus.format
            )
            if renderStatePtr.pointee.handle == nil
                || inputChannels != renderStatePtr.pointee.inputChannels
                || outputChannels != renderStatePtr.pointee.outputChannels
                || sampleRate != rustSampleRate
                || configuration != rustConfiguration
                || maximumFramesToRender != rustMaximumFramesToRender {
                try createRustPluginLocked(
                    inputFormat: inputBus.format,
                    outputFormat: outputBus.format,
                    sampleRate: sampleRate
                )
                // Rebuild atomically whenever the replacement's schema truly
                // differs, including config/sample-rate driven differences.
                // Preserve the existing AUParameter objects when it matches.
                if parameterSchemaMatchesLocked() {
                    advanceParameterEpochLocked()
                } else {
                    buildParameterTreeLocked()
                }
            }

            oldScratchIn = renderStatePtr.pointee.scratchIn
            oldScratchOut = renderStatePtr.pointee.scratchOut
            oldOwnedOutput = renderStatePtr.pointee.ownedOutput
            oldMidiIn = renderStatePtr.pointee.midiIn
            oldMidiOut = renderStatePtr.pointee.midiOut
            oldNoteOut = renderStatePtr.pointee.noteExpressionOut
            oldPullBuffer = renderStateOwner.inputPullBuffer
            renderStatePtr.pointee.scratchIn = candidateScratchIn
            renderStatePtr.pointee.scratchOut = candidateScratchOut
            renderStatePtr.pointee.ownedOutput = candidateOwnedOutput
            renderStatePtr.pointee.scratchCapacity = needed
            renderStatePtr.pointee.maximumFrames = maximumFramesToRender
            renderStateOwner.inputPullBuffer = candidatePullBuffer
            renderStatePtr.pointee.inputBufferList = candidatePullBuffer.mutableAudioBufferList
            renderStatePtr.pointee.midiIn = candidateMidiIn
            renderStatePtr.pointee.midiOut = candidateMidiOut
            renderStatePtr.pointee.noteExpressionOut = candidateNoteOut
            renderStatePtr.pointee.eventCapacity = eventCapacity
            renderStatePtr.pointee.resourcesAllocated = true
            candidateCommitted = true
        }
        // The gate guarantees no render can still hold the retired bundle.
        oldScratchIn?.deallocate()
        oldScratchOut?.deallocate()
        oldOwnedOutput?.deallocate()
        oldMidiIn?.deallocate()
        oldMidiOut?.deallocate()
        oldNoteOut?.deallocate()
        _ = oldPullBuffer
        completed = true
    }

    public override func deallocateRenderResources() {
        var scratchIn: UnsafeMutablePointer<Float>?
        var scratchOut: UnsafeMutablePointer<Float>?
        var ownedOutput: UnsafeMutablePointer<Float>?
        var midiIn: UnsafeMutablePointer<PluginMidiEvent>?
        var midiOut: UnsafeMutablePointer<PluginMidiEvent>?
        var noteOut: UnsafeMutablePointer<PluginNoteExpressionEvent>?
        var pullBuffer: AVAudioPCMBuffer?
        accessGate.withControlAccess {
            renderStatePtr.pointee.resourcesAllocated = false
            scratchIn = renderStatePtr.pointee.scratchIn
            scratchOut = renderStatePtr.pointee.scratchOut
            ownedOutput = renderStatePtr.pointee.ownedOutput
            midiIn = renderStatePtr.pointee.midiIn
            midiOut = renderStatePtr.pointee.midiOut
            noteOut = renderStatePtr.pointee.noteExpressionOut
            pullBuffer = renderStateOwner.inputPullBuffer
            renderStatePtr.pointee.scratchIn = nil
            renderStatePtr.pointee.scratchOut = nil
            renderStatePtr.pointee.ownedOutput = nil
            renderStatePtr.pointee.inputBufferList = nil
            renderStatePtr.pointee.midiIn = nil
            renderStatePtr.pointee.midiOut = nil
            renderStatePtr.pointee.noteExpressionOut = nil
            renderStatePtr.pointee.scratchCapacity = 0
            renderStatePtr.pointee.maximumFrames = 0
            renderStatePtr.pointee.eventCapacity = 0
            renderStateOwner.inputPullBuffer = nil
        }
        scratchIn?.deallocate()
        scratchOut?.deallocate()
        ownedOutput?.deallocate()
        midiIn?.deallocate()
        midiOut?.deallocate()
        noteOut?.deallocate()
        _ = pullBuffer
        super.deallocateRenderResources()
    }

    // MARK: - Audio Processing

    public override var internalRenderBlock: AUInternalRenderBlock {
        // Capture the POINTER, not the values. The pointed-to struct is updated
        // by allocateRenderResources, so the render thread always sees current state.
        let stateOwner = renderStateOwner
        let statePtr = stateOwner.pointer
        let gate = accessGate
        let midiOutputBlock: AUMIDIOutputEventBlock? = midiOutputNames.isEmpty
            ? nil
            : midiOutputEventBlock

        return { (
            actionFlags,
            timestamp,
            frameCount,
            outputBusNumber,
            outputData,
            realtimeEventListHead,
            pullInputBlock
        ) in
            #if SOTF_NATIVE_SMOKE
            sotfRenderCounterEnter()
            defer { sotfRenderCounterLeave() }
            #endif
            _ = stateOwner
            // OutputIsSilence is an output claim. Never carry a stale host bit
            // into a block which this effect is about to overwrite.
            let incomingActionFlags = actionFlags.pointee.rawValue
            actionFlags.pointee = AudioUnitRenderActionFlags(
                rawValue: incomingActionFlags & ~(1 << 4)
            )
            let skipRenderArgumentChecks = incomingActionFlags & (1 << 9) != 0
            guard gate.tryAcquireRender() else {
                return kAudioUnitErr_CannotDoInCurrentContext
            }
            defer { gate.release() }

            let state = statePtr.pointee
            guard state.resourcesAllocated else { return kAudioUnitErr_Uninitialized }
            guard let pullInputBlock = pullInputBlock, let handle = state.handle else {
                return kAudioUnitErr_NoConnection
            }

            let inputChannels = state.inputChannels
            let outputChannels = state.outputChannels
            guard inputChannels > 0, outputChannels > 0,
                  let inputBufferList = state.inputBufferList else {
                return kAudioUnitErr_Uninitialized
            }
            if let ids = state.parameterIds,
               let mailboxes = state.parameterMailboxes {
                var accepted = true
                for address in 0..<state.parameterCount {
                    let mailbox = mailboxes[address].takeUnretainedValue()
                    guard let pending = mailbox.pendingValue() else { continue }
                    mailbox.markApplied(pending.version)
                    guard pending.value.isFinite,
                          (0.0...1.0).contains(pending.value),
                          let id = ids[address],
                          plugin_set_parameter(handle, id, Double(pending.value)) == 0 else {
                        accepted = false
                        continue
                    }
                }
                if !accepted { return kAudioUnitErr_InvalidParameter }
            }
            // Native AU parameter events already arrive on the render owner.
            // Apply point events and bounded ramps at their segment boundaries
            // without allocating or taking a producer-owned lock. Rust plugins
            // receive the target value before processing the remaining segment;
            // the event ordering and sample offsets are preserved by splitting
            // the block below.
            var parameterEvent = realtimeEventListHead
            var segmentStart = 0

            let frames = Int(frameCount)
            let rawSampleTime = timestamp.pointee.mSampleTime
            let sampleTimeValid = timestamp.pointee.mFlags.contains(.sampleTimeValid)
                && rawSampleTime.isFinite
                && rawSampleTime >= Double(AUEventSampleTime.min)
                && rawSampleTime <= Double(AUEventSampleTime.max)
            let blockSampleTime = sampleTimeValid
                ? AUEventSampleTime(timestamp.pointee.mSampleTime.rounded(.towardZero))
                : 0
            let inputSampleCount = frames.multipliedReportingOverflow(by: inputChannels)
            let outputSampleCount = frames.multipliedReportingOverflow(by: outputChannels)
            guard outputBusNumber == 0,
                  frameCount <= state.maximumFrames,
                  !inputSampleCount.overflow,
                  !outputSampleCount.overflow,
                  inputSampleCount.partialValue <= state.scratchCapacity,
                  outputSampleCount.partialValue <= state.scratchCapacity else {
                return kAudioUnitErr_TooManyFramesToProcess
            }

            let outputBufferList = UnsafeMutableAudioBufferListPointer(outputData)
            if outputBufferList.count == 1 {
                let buffer = outputBufferList[0]
                guard (skipRenderArgumentChecks
                        || buffer.mNumberChannels == UInt32(outputChannels)),
                      buffer.mData == nil
                        || skipRenderArgumentChecks
                        || Int(buffer.mDataByteSize)
                            >= outputSampleCount.partialValue * MemoryLayout<Float>.size else {
                    return kAudioUnitErr_InvalidPropertyValue
                }
            } else {
                guard outputBufferList.count == outputChannels else {
                    return kAudioUnitErr_InvalidPropertyValue
                }
                let bytesPerChannel = frames * MemoryLayout<Float>.size
                for channel in 0..<outputChannels {
                    let buffer = outputBufferList[channel]
                    guard (skipRenderArgumentChecks || buffer.mNumberChannels == 1),
                          buffer.mData == nil || skipRenderArgumentChecks
                            || Int(buffer.mDataByteSize) >= bytesPerChannel else {
                        return kAudioUnitErr_InvalidPropertyValue
                    }
                }
            }

            // Pull input audio
            var pullFlags = AudioUnitRenderActionFlags(rawValue: 0)
            let status = pullInputBlock(&pullFlags, timestamp, frameCount, 0, inputBufferList)
            guard status == noErr else { return status }

            guard let scratchIn = state.scratchIn, let scratchOut = state.scratchOut else {
                return kAudioUnitErr_Uninitialized
            }

            let inputBuffers = UnsafeMutableAudioBufferListPointer(inputBufferList)

            if inputBuffers.count == 1 {
                let buffer = inputBuffers[0]
                guard (skipRenderArgumentChecks
                        || buffer.mNumberChannels == UInt32(inputChannels)),
                      buffer.mData != nil,
                      skipRenderArgumentChecks
                        ||
                      Int(buffer.mDataByteSize) >= inputSampleCount.partialValue * MemoryLayout<Float>.size else {
                    return kAudioUnitErr_InvalidPropertyValue
                }
            } else {
                guard inputBuffers.count == inputChannels else {
                    return kAudioUnitErr_InvalidPropertyValue
                }
                let bytesPerChannel = frames * MemoryLayout<Float>.size
                for channel in 0..<inputChannels {
                    let buffer = inputBuffers[channel]
                    guard (skipRenderArgumentChecks || buffer.mNumberChannels == 1),
                          buffer.mData != nil,
                          skipRenderArgumentChecks || Int(buffer.mDataByteSize) >= bytesPerChannel else {
                        return kAudioUnitErr_InvalidPropertyValue
                    }
                }
            }

            // Interleave input from AU's deinterleaved buffers
            if inputBuffers.count == 1 && inputBuffers[0].mNumberChannels == UInt32(inputChannels) {
                if let mData = inputBuffers[0].mData {
                    let src = mData.assumingMemoryBound(to: Float.self)
                    scratchIn.update(from: src, count: frames * inputChannels)
                }
            } else {
                for ch in 0..<inputChannels {
                    guard let mData = inputBuffers[ch].mData else {
                        return kAudioUnitErr_InvalidPropertyValue
                    }
                    let src = mData.assumingMemoryBound(to: Float.self)
                    for frame in 0..<frames {
                        scratchIn[frame * inputChannels + ch] = src[frame]
                    }
                }
            }

            var hasActiveRamp = false
            if let ramps = state.parameterRamps {
                for address in 0..<state.parameterCount where ramps[address].active {
                    hasActiveRamp = true
                    break
                }
            }
            if realtimeEventListHead == nil && !hasActiveRamp && midiOutputBlock == nil {
                guard plugin_process(handle, scratchIn, scratchOut, frames) == 0 else {
                    return kAudioUnitErr_FailedInitialization
                }
                if outputBufferList.count == 1 {
                    let byteCount = outputSampleCount.partialValue * MemoryLayout<Float>.size
                    if let mData = outputBufferList[0].mData {
                        mData.assumingMemoryBound(to: Float.self)
                            .update(from: scratchOut, count: frames * outputChannels)
                    } else {
                        outputBufferList[0].mData = UnsafeMutableRawPointer(scratchOut)
                    }
                    outputBufferList[0].mDataByteSize = UInt32(byteCount)
                } else {
                    guard let ownedOutput = state.ownedOutput else {
                        return kAudioUnitErr_Uninitialized
                    }
                    for ch in 0..<outputChannels {
                        let dst: UnsafeMutablePointer<Float>
                        if let mData = outputBufferList[ch].mData {
                            dst = mData.assumingMemoryBound(to: Float.self)
                        } else {
                            dst = ownedOutput.advanced(by: ch * frames)
                            outputBufferList[ch].mData = UnsafeMutableRawPointer(dst)
                        }
                        for frame in 0..<frames {
                            dst[frame] = scratchOut[frame * outputChannels + ch]
                        }
                        outputBufferList[ch].mDataByteSize = UInt32(
                            frames * MemoryLayout<Float>.size
                        )
                    }
                }
                return noErr
            }

            // Process through Rust plugin and copy any queued MIDI/Note Expression output events.
            let copiedMIDI = Self.copyMIDIInputEvents(
                from: realtimeEventListHead,
                to: state.midiIn,
                capacity: state.eventCapacity,
                frameCount: frames,
                blockSampleTime: blockSampleTime,
                sampleTimeValid: sampleTimeValid
            )
            guard !copiedMIDI.overflow else { return kAudioUnitErr_InvalidParameter }
            var segmentProcessor = RenderSegmentProcessor(
                state: state,
                handle: handle,
                scratchIn: scratchIn,
                scratchOut: scratchOut,
                inputChannels: inputChannels,
                outputChannels: outputChannels,
                copiedMIDIInputCount: copiedMIDI.count,
                midiOutputEnabled: midiOutputBlock != nil
            )
            var parameterEventCount = 0
            while let current = parameterEvent {
                let head = current.pointee.head
                if head.eventType == .parameter || head.eventType == .parameterRamp {
                    parameterEventCount += 1
                    guard parameterEventCount <= state.eventCapacity else {
                        return kAudioUnitErr_InvalidParameter
                    }
                    let event = current.pointee.parameter
                    let offset = Self.relativeEventOffset(
                        eventSampleTime: event.eventSampleTime,
                        blockSampleTime: blockSampleTime,
                        sampleTimeValid: sampleTimeValid,
                        maximumOffset: frames
                    )
                    guard offset >= segmentStart,
                          event.parameterAddress < UInt64(state.parameterCount),
                          let ids = state.parameterIds,
                          let id = ids[Int(event.parameterAddress)] else {
                        return kAudioUnitErr_InvalidParameter
                    }
                    guard segmentProcessor.processWithActiveRamps(
                        start: segmentStart,
                        count: offset - segmentStart
                    ) == 0 else {
                        return kAudioUnitErr_FailedInitialization
                    }
                    // The AU event's value is in host units; normalize with the
                    // same cached AU metadata used by UI automation.
                    let address = Int(event.parameterAddress)
                    guard let metadata = state.parameterMetadata else {
                        return kAudioUnitErr_InvalidParameter
                    }
                    let info = metadata[address]
                    let minValue = info.minValue
                    let maxValue = info.maxValue
                    let rawValue = Double(event.value)
                    let range = maxValue - minValue
                    let normalized: Double
                    if range <= 0 {
                        normalized = 0
                    } else if info.logarithmic && minValue > 0 {
                        normalized = (log(max(rawValue, minValue)) - log(minValue))
                            / (log(maxValue) - log(minValue))
                    } else {
                        normalized = (rawValue - minValue) / range
                    }
                    if head.eventType == .parameterRamp,
                       event.rampDurationSampleFrames > 0,
                       let ramps = state.parameterRamps {
                        let startValue = plugin_get_parameter(handle, id)
                        guard startValue >= 0 else { return kAudioUnitErr_InvalidParameter }
                        let totalDuration = Int(event.rampDurationSampleFrames)
                        ramps[address] = ParameterRampState(
                            active: true,
                            current: startValue,
                            target: normalized,
                            step: (normalized - startValue) / Double(totalDuration),
                            remaining: totalDuration
                        )
                        segmentStart = offset
                    } else {
                        if let ramps = state.parameterRamps {
                            ramps[address] = ParameterRampState()
                        }
                        guard plugin_set_parameter(handle, id, normalized) == 0 else {
                            return kAudioUnitErr_InvalidParameter
                        }
                        segmentStart = offset
                    }
                }
                parameterEvent = UnsafePointer(head.next)
            }
            guard segmentProcessor.processWithActiveRamps(
                start: segmentStart,
                count: frames - segmentStart
            ) == 0 else {
                return OSStatus(kAudioUnitErr_FailedInitialization)
            }

            if let midiOutputBlock = midiOutputBlock, let midiOut = state.midiOut {
                for i in 0..<segmentProcessor.midiOutputCount {
                    var event = midiOut[i]
                    let callbackStatus: OSStatus = withUnsafeBytes(of: &event.data) { bytes in
                        if let base = bytes.baseAddress {
                            return midiOutputBlock(
                                Self.absoluteEventTime(
                                    sampleOffset: event.sample_offset,
                                    blockSampleTime: blockSampleTime,
                                    sampleTimeValid: sampleTimeValid
                                ),
                                0,
                                Int(event.len),
                                base.assumingMemoryBound(to: UInt8.self)
                            )
                        }
                        return kAudioUnitErr_InvalidParameter
                    }
                    guard callbackStatus == noErr else { return callbackStatus }
                }
            }
            // AUv3 exposes no callback through which this `aufx` wrapper can
            // faithfully publish note-expression output. Never silently drain
            // and discard such events.
            guard segmentProcessor.noteExpressionOutputCount == 0 else {
                return kAudioUnitErr_InvalidParameter
            }

            // Deinterleave output back to AU's buffers
            if outputBufferList.count == 1 {
                let byteCount = outputSampleCount.partialValue * MemoryLayout<Float>.size
                if let mData = outputBufferList[0].mData {
                    let dst = mData.assumingMemoryBound(to: Float.self)
                    dst.update(from: scratchOut, count: frames * outputChannels)
                } else {
                    outputBufferList[0].mData = UnsafeMutableRawPointer(scratchOut)
                }
                outputBufferList[0].mDataByteSize = UInt32(byteCount)
            } else {
                guard let ownedOutput = state.ownedOutput else {
                    return kAudioUnitErr_Uninitialized
                }
                for ch in 0..<outputChannels {
                    let dst: UnsafeMutablePointer<Float>
                    if let mData = outputBufferList[ch].mData {
                        dst = mData.assumingMemoryBound(to: Float.self)
                    } else {
                        dst = ownedOutput.advanced(by: ch * frames)
                        outputBufferList[ch].mData = UnsafeMutableRawPointer(dst)
                    }
                    for frame in 0..<frames {
                        dst[frame] = scratchOut[frame * outputChannels + ch]
                    }
                    outputBufferList[ch].mDataByteSize = UInt32(frames * MemoryLayout<Float>.size)
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
        // `plugin_ffi_capabilities` describes the bridge ABI, not this plugin.
        // Current native wrappers are audio effects (`aufx`) and make no
        // per-plugin note-expression contract.
        false
    }

    public override var midiOutputNames: [String] {
        // Do not turn a global ABI capability into a false per-AU promise.
        []
    }

    public override var fullState: [String: Any]? {
        get {
            guard let snapshot: (Data, [(String, AUValue)]) = accessGate.withControlAccess({
                guard let handle = renderStatePtr.pointee.handle else { return nil }
                guard drainParameterMailboxesLocked(handle: handle) else { return nil }
                var len: Int = 0
                guard let data = plugin_save_state(handle, &len), len > 0 else { return nil }
                defer { plugin_free_state(data, len) }
                let values = auParameters.map { parameter -> (String, AUValue) in
                    let normalized = parameter.identifier.withCString {
                        plugin_get_parameter(handle, $0)
                    }
                    return (
                        parameter.identifier,
                        normalized >= 0
                            ? denormalize(normalized: Float(normalized), param: parameter)
                            : parameter.minValue
                    )
                }
                return (Data(bytes: data, count: len), values)
            }) else { return nil }

            var state: [String: Any] = [
                kAUPresetTypeKey: FourCharCode(kAudioUnitType_Effect) as NSNumber,
                kAUPresetSubtypeKey: fourCharCode(type(of: self).pluginSubtype) as NSNumber,
                kAUPresetManufacturerKey: fourCharCode("SOTF") as NSNumber,
                kAUPresetVersionKey: 1 as NSNumber,
                kAUPresetNameKey: "Default",
                "sotf_state": snapshot.0,
            ]

            for (identifier, value) in snapshot.1 {
                state[identifier] = value
            }

            return state
        }
        set {
            guard let state = newValue else { return }

            if let data = state["sotf_state"] as? Data {
                var parameterValues: [(identifier: String, normalized: Double)] = []
                var invalidParameterValue = false
                for parameter in auParameters {
                    guard let raw = state[parameter.identifier] else { continue }
                    guard let number = raw as? NSNumber else {
                        invalidParameterValue = true
                        break
                    }
                    let normalized = normalize(value: number.floatValue, param: parameter)
                    guard normalized.isFinite, (0.0...1.0).contains(normalized) else {
                        invalidParameterValue = true
                        break
                    }
                    parameterValues.append((parameter.identifier, Double(normalized)))
                }
                guard !invalidParameterValue else { return }
                willChangeValue(forKey: "allParameterValues")
                willChangeValue(forKey: "latency")
                defer {
                    didChangeValue(forKey: "latency")
                    didChangeValue(forKey: "allParameterValues")
                }
                let loaded = accessGate.withControlAccess {
                    replacePluginStateLocked(data, parameterValues: parameterValues)
                }
                guard loaded else { return }
                return
            }

            for param in auParameters {
                if let number = state[param.identifier] as? NSNumber {
                    let value = number.floatValue
                    param.value = value
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
        accessGate.withControlAccess {
            guard let handle = renderStatePtr.pointee.handle else { return nil }
            guard drainParameterMailboxesLocked(handle: handle) else { return nil }
            var len = 0
            let ptr = name.withCString { namePtr in
                plugin_export_preset_json(handle, namePtr, &len)
            }
            guard let ptr = ptr, len > 0 else { return nil }
            defer { plugin_free_state(ptr, len) }
            return Data(bytes: ptr, count: len)
        }
    }

    public func importPresetDocument(_ data: Data) -> Bool {
        willChangeValue(forKey: "allParameterValues")
        willChangeValue(forKey: "latency")
        defer {
            didChangeValue(forKey: "latency")
            didChangeValue(forKey: "allParameterValues")
        }
        return accessGate.withControlAccess { replacePluginPresetDocumentLocked(data) }
    }

    public func suggestedPresetFilename(named name: String) -> String? {
        accessGate.withControlAccess {
            guard let handle = renderStatePtr.pointee.handle else { return nil }
            let ptr = name.withCString { namePtr in
                plugin_suggest_preset_filename(handle, namePtr)
            }
            guard let ptr = ptr else { return nil }
            defer { plugin_free_string(ptr) }
            return String(cString: ptr)
        }
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
        frameCount: Int,
        blockSampleTime: AUEventSampleTime,
        sampleTimeValid: Bool
    ) -> (count: Int, overflow: Bool) {
        guard let buffer = buffer, capacity > 0 else {
            return (0, eventList != nil)
        }

        var count = 0
        var overflow = false
        var event = eventList
        while let current = event {
            let head = current.pointee.head
            if head.eventType == .MIDI {
                let midi = current.pointee.MIDI
                if midi.length > 0 && midi.length <= 3 {
                    let sampleOffset = relativeEventOffset(
                        eventSampleTime: midi.eventSampleTime,
                        blockSampleTime: blockSampleTime,
                        sampleTimeValid: sampleTimeValid,
                        maximumOffset: max(frameCount - 1, 0)
                    )
                    if count < capacity {
                        buffer[count] = PluginMidiEvent(
                            sample_offset: sampleOffset,
                            data: (midi.data.0, midi.data.1, midi.data.2),
                            len: UInt8(midi.length)
                        )
                        count += 1
                    } else {
                        overflow = true
                    }
                }
            }
            event = UnsafePointer(head.next)
        }
        return (count, overflow)
    }

    @inline(__always)
    private static func relativeEventOffset(
        eventSampleTime: AUEventSampleTime,
        blockSampleTime: AUEventSampleTime,
        sampleTimeValid: Bool,
        maximumOffset: Int
    ) -> Int {
        // Negative AU event times mean "immediate". Otherwise event times
        // share the absolute AU timeline whenever the render timestamp says
        // its sample time is valid.
        if eventSampleTime < 0 { return 0 }
        let relative: AUEventSampleTime
        if sampleTimeValid {
            let translated = eventSampleTime.subtractingReportingOverflow(blockSampleTime)
            if translated.overflow { return eventSampleTime < blockSampleTime ? 0 : maximumOffset }
            relative = translated.partialValue
        } else {
            relative = eventSampleTime
        }
        if relative <= 0 { return 0 }
        if relative >= AUEventSampleTime(maximumOffset) { return maximumOffset }
        return Int(relative)
    }

    @inline(__always)
    private static func absoluteEventTime(
        sampleOffset: Int,
        blockSampleTime: AUEventSampleTime,
        sampleTimeValid: Bool
    ) -> AUEventSampleTime {
        let base = sampleTimeValid ? blockSampleTime : 0
        let translated = base.addingReportingOverflow(AUEventSampleTime(sampleOffset))
        return translated.overflow ? AUEventSampleTime.max : translated.partialValue
    }

    #if SOTF_NATIVE_SMOKE
    static func eventTimelineForTesting(
        eventTime: AUEventSampleTime,
        blockStart: AUEventSampleTime,
        valid: Bool,
        maximumOffset: Int
    ) -> (offset: Int, outputTime: AUEventSampleTime) {
        let offset = relativeEventOffset(
            eventSampleTime: eventTime,
            blockSampleTime: blockStart,
            sampleTimeValid: valid,
            maximumOffset: maximumOffset
        )
        return (offset, absoluteEventTime(
            sampleOffset: offset,
            blockSampleTime: blockStart,
            sampleTimeValid: valid
        ))
    }
    #endif
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
