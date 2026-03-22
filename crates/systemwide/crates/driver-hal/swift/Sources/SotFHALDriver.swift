// SotF HAL Driver - Swift Implementation
// A Core Audio HAL plugin that creates a virtual audio device
// Phases 1-5: Full implementation with shared memory for Rust engine integration

import Foundation
import CoreAudio
import os.log

// MARK: - Logging

private let logger = OSLog(subsystem: "org.spinorama.sotf-hal", category: "HALDriver")

func halLog(_ message: String) {
    os_log("%{public}@", log: logger, type: .error, message)
    NSLog("SotF: %@", message)
}

private func fourCC(_ value: UInt32) -> String {
    let chars = [
        Character(UnicodeScalar((value >> 24) & 0xFF)!),
        Character(UnicodeScalar((value >> 16) & 0xFF)!),
        Character(UnicodeScalar((value >> 8) & 0xFF)!),
        Character(UnicodeScalar(value & 0xFF)!)
    ]
    return String(chars)
}

// MARK: - Object IDs

private let kPlugInObjectID: AudioObjectID = 1
private let kDeviceObjectID: AudioObjectID = 2
private let kInputStreamObjectID: AudioObjectID = 3
private let kOutputStreamObjectID: AudioObjectID = 4
// Future: kVolumeControlObjectID: AudioObjectID = 5

// MARK: - Property Selectors (FourCC codes)

// Object properties
private let kSelector_BaseClass: UInt32           = 0x62636C73  // 'bcls'
private let kSelector_Class: UInt32               = 0x636C6173  // 'clas'
private let kSelector_Owner: UInt32               = 0x73746476  // 'stdv'
private let kSelector_Name: UInt32                = 0x6C6E616D  // 'lnam'
private let kSelector_Manufacturer: UInt32        = 0x6C6D616B  // 'lmak'
private let kSelector_OwnedObjects: UInt32        = 0x6F776E64  // 'ownd'
private let kSelector_Identify: UInt32            = 0x6964656E  // 'iden'
private let kSelector_SerialNumber: UInt32        = 0x736E756D  // 'snum'
private let kSelector_FirmwareVersion: UInt32     = 0x6677766E  // 'fwvn'
private let kSelector_ControlList: UInt32         = 0x6374726C  // 'ctrl'
private let kSelector_CustomPropertyInfo: UInt32  = 0x63757374  // 'cust'

// Plugin properties
private let kSelector_BundleID: UInt32            = 0x70696964  // 'piid'
private let kSelector_DeviceList: UInt32          = 0x64657623  // 'dev#'
private let kSelector_ResourceBundle: UInt32      = 0x72737263  // 'rsrc'
private let kSelector_TranslateUID: UInt32        = 0x75696464  // 'uidd'
private let kSelector_BoxList: UInt32             = 0x626F7823  // 'box#'

// Device properties
private let kSelector_DeviceUID: UInt32           = 0x75696420  // 'uid '
private let kSelector_ModelUID: UInt32            = 0x6D756964  // 'muid'
private let kSelector_TransportType: UInt32       = 0x7472616E  // 'tran'
private let kSelector_RelatedDevices: UInt32      = 0x616B696E  // 'akin'
private let kSelector_ClockDomain: UInt32         = 0x636C6B64  // 'clkd'
private let kSelector_DeviceIsAlive: UInt32       = 0x6C69766E  // 'livn'
private let kSelector_DeviceIsRunning: UInt32     = 0x676F696E  // 'goin'
private let kSelector_CanBeDefault: UInt32        = 0x64666C74  // 'dflt'
private let kSelector_CanBeSystemDefault: UInt32  = 0x73666C74  // 'sflt'
private let kSelector_Latency: UInt32             = 0x6C746E63  // 'ltnc'
private let kSelector_Streams: UInt32             = 0x73746D23  // 'stm#'
private let kSelector_SafetyOffset: UInt32        = 0x73616674  // 'saft'
private let kSelector_NominalSampleRate: UInt32   = 0x6E737274  // 'nsrt'
private let kSelector_AvailableSampleRates: UInt32 = 0x6E737223 // 'nsr#'
private let kSelector_Icon: UInt32                = 0x69636F6E  // 'icon'
private let kSelector_IsHidden: UInt32            = 0x6869646E  // 'hidn'
private let kSelector_PreferredStereo: UInt32     = 0x64636832  // 'dch2'
private let kSelector_PreferredLayout: UInt32     = 0x73726E64  // 'srnd'
private let kSelector_ZeroTimePeriod: UInt32      = 0x72696E67  // 'ring'
private let kSelector_ClockAlgorithm: UInt32      = 0x636C6F6B  // 'clok'
private let kSelector_ClockIsStable: UInt32       = 0x63737462  // 'cstb'
private let kSelector_BufferFrameSize: UInt32     = 0x6673697A  // 'fsiz'
private let kSelector_BufferSizeRange: UInt32     = 0x66737A23  // 'fsz#'
private let kSelector_StreamConfig: UInt32        = 0x736C6179  // 'slay'
private let kSelector_ConfigApp: UInt32           = 0x63617070  // 'capp'
private let kSelector_DeviceCanBeDefault: UInt32  = 0x64666C74  // 'dflt' (same as CanBeDefault)

// Stream properties
private let kSelector_StreamIsActive: UInt32      = 0x73616374  // 'sact'
private let kSelector_StreamDirection: UInt32     = 0x73646972  // 'sdir'
private let kSelector_TerminalType: UInt32        = 0x7465726D  // 'term'
private let kSelector_StartingChannel: UInt32     = 0x7363686E  // 'schn'
private let kSelector_VirtualFormat: UInt32       = 0x73666D74  // 'sfmt'
private let kSelector_AvailableVirtualFmts: UInt32 = 0x73666D61 // 'sfma'
private let kSelector_PhysicalFormat: UInt32      = 0x70667420  // 'pft '
private let kSelector_AvailablePhysicalFmts: UInt32 = 0x70667461 // 'pfta'

// Class IDs
private let kClassID_Object: UInt32               = 0x616F626A  // 'aobj'
private let kClassID_PlugIn: UInt32               = 0x61706C67  // 'aplg'
private let kClassID_Device: UInt32               = 0x61646576  // 'adev'
private let kClassID_Stream: UInt32               = 0x61737472  // 'astr'
private let kClassID_Control: UInt32              = 0x6163746C  // 'actl'

// Transport types
private let kTransport_Virtual: UInt32            = 0x76697274  // 'virt'

// Terminal types
private let kTerminal_Microphone: UInt32          = 0x6D696372  // 'micr'
private let kTerminal_Speaker: UInt32             = 0x73706B72  // 'spkr'

// Scope constants
private let kScope_Global: UInt32                 = 0x676C6F62  // 'glob'
private let kScope_Input: UInt32                  = 0x696E7074  // 'inpt'
private let kScope_Output: UInt32                 = 0x6F757470  // 'outp'
private let kScope_Wildcard: UInt32               = 0x2A2A2A2A  // '****'

// IO Operation IDs (FourCC codes from AudioServerPlugIn.h)
private let kIOOperation_Thread: UInt32 = 0x74687264       // 'thrd' - Thread begin/end
private let kIOOperation_Cycle: UInt32 = 0x6379636C        // 'cycl' - IO cycle begin/end
private let kIOOperation_ReadInput: UInt32 = 0x72656164    // 'read' - Read from input device
private let kIOOperation_ConvertInput: UInt32 = 0x63696E70 // 'cinp' - Convert input format
private let kIOOperation_ProcessInput: UInt32 = 0x70696E70 // 'pinp' - Process input (DSP)
private let kIOOperation_ProcessOutput: UInt32 = 0x706F7574 // 'pout' - Process output (DSP)
private let kIOOperation_MixOutput: UInt32 = 0x6D69786F    // 'mixo' - Mix output streams
private let kIOOperation_ProcessMix: UInt32 = 0x706D6978   // 'pmix' - Process mixed output
private let kIOOperation_ConvertMix: UInt32 = 0x636D6978   // 'cmix' - Convert mix format
private let kIOOperation_WriteMix: UInt32 = 0x72697465     // 'rite' - Write mixed output

// MARK: - Supported Sample Rates

private let kSupportedSampleRates: [Float64] = [44100.0, 48000.0, 88200.0, 96000.0, 176400.0, 192000.0]

// MARK: - Driver State

final class DriverState {
    var host: AudioServerPlugInHostRef?
    var sampleRate: Float64 = 48000.0
    var bufferFrameSize: UInt32 = 512
    var channelCount: UInt32 = 2

    // IO client count - accessed atomically for thread safety
    // Multiple CoreAudio threads can call StartIO/StopIO concurrently
    // Using fileprivate to allow atomic operations from driver callbacks
    fileprivate var _ioClientCount: Int32 = 0

    var ioClientCount: Int32 {
        get { OSAtomicAdd32(0, &_ioClientCount) }
    }

    func incrementIOClientCount() -> Int32 {
        return OSAtomicIncrement32(&_ioClientCount)
    }

    func decrementIOClientCount() -> Int32 {
        return OSAtomicDecrement32(&_ioClientCount)
    }

    func resetIOClientCount() {
        // Reset to 0 atomically
        while true {
            let current = _ioClientCount
            if OSAtomicCompareAndSwap32(current, 0, &_ioClientCount) {
                break
            }
        }
    }

    // Timing
    let clock = DriverClock()

    // Audio buffers
    var inputRingBuffer: MultiChannelRingBuffer?
    var outputRingBuffer: MultiChannelRingBuffer?

    // Shared memory for Rust engine
    let sharedAudio = SharedAudioBuffer()

    // Loopback mode (when Rust engine not connected)
    var loopbackEnabled: Bool = true

    // Flag to indicate driver is being disposed (prevents race conditions in StartIO)
    private var _isDisposed: Int32 = 0
    var isDisposed: Bool {
        get { OSAtomicAdd32(0, &_isDisposed) != 0 }
        set { OSAtomicCompareAndSwap32(_isDisposed, newValue ? 1 : 0, &_isDisposed) }
    }

    static let shared = DriverState()
    private init() {
        // Initialize ring buffers
        resetBuffers()
    }

    func resetBuffers() {
        let capacity = Int(bufferFrameSize) * 16  // 16 buffers worth
        inputRingBuffer = MultiChannelRingBuffer(channelCount: Int(channelCount), framesCapacity: capacity)
        outputRingBuffer = MultiChannelRingBuffer(channelCount: Int(channelCount), framesCapacity: capacity)
    }

    var isRunning: Bool {
        return ioClientCount > 0
    }
}

// MARK: - Helper Functions

private func createCFString(_ string: String) -> Unmanaged<CFString> {
    return Unmanaged.passRetained(string as CFString)
}

private func getObjectType(_ objectID: AudioObjectID) -> String {
    switch objectID {
    case kPlugInObjectID: return "plugin"
    case kDeviceObjectID: return "device"
    case kInputStreamObjectID: return "input_stream"
    case kOutputStreamObjectID: return "output_stream"
    default: return "unknown(\(objectID))"
    }
}

private func scopeMatches(_ queryScope: UInt32, _ targetScope: UInt32) -> Bool {
    return queryScope == kScope_Wildcard || queryScope == targetScope || queryScope == kScope_Global
}

// MARK: - Driver Callbacks

private func driverInitialize(
    _ driver: AudioServerPlugInDriverRef,
    _ host: AudioServerPlugInHostRef
) -> OSStatus {
    halLog("Initialize called - VERSION 2026-02-01-A")
    DriverState.shared.host = host

    // Initialize shared memory for Rust engine communication
    let state = DriverState.shared
    halLog("Initializing SharedMemory: sampleRate=\(state.sampleRate), bufferFrames=\(state.bufferFrameSize), channels=\(state.channelCount)")

    let success = state.sharedAudio.initialize(
        sampleRate: UInt32(state.sampleRate),
        bufferFrames: state.bufferFrameSize,
        channelCount: state.channelCount
    )

    if success {
        halLog("Shared memory initialized successfully")
        halLog("SharedMemory state after init: \(state.sharedAudio.connectionStateDebug)")
    } else {
        halLog("ERROR: Shared memory init failed, using loopback mode only")
    }

    halLog("Initialize complete - isConnected=\(state.sharedAudio.isConnected), engineReady=\(state.sharedAudio.engineReady)")
    return noErr
}

private func driverCreateDevice(
    _ driver: AudioServerPlugInDriverRef,
    _ description: CFDictionary,
    _ clientInfo: UnsafePointer<AudioServerPlugInClientInfo>,
    _ deviceObjectID: UnsafeMutablePointer<AudioObjectID>
) -> OSStatus {
    halLog("CreateDevice called")
    deviceObjectID.pointee = kDeviceObjectID
    return noErr
}

private func driverDestroyDevice(_ driver: AudioServerPlugInDriverRef, _ deviceObjectID: AudioObjectID) -> OSStatus {
    halLog("DestroyDevice: \(deviceObjectID)")
    return noErr
}

private func driverAddDeviceClient(_ driver: AudioServerPlugInDriverRef, _ deviceObjectID: AudioObjectID, _ clientInfo: UnsafePointer<AudioServerPlugInClientInfo>) -> OSStatus {
    halLog("AddDeviceClient: device=\(deviceObjectID)")
    return noErr
}

private func driverRemoveDeviceClient(_ driver: AudioServerPlugInDriverRef, _ deviceObjectID: AudioObjectID, _ clientInfo: UnsafePointer<AudioServerPlugInClientInfo>) -> OSStatus {
    halLog("RemoveDeviceClient: device=\(deviceObjectID)")
    return noErr
}

private func driverPerformDeviceConfigurationChange(_ driver: AudioServerPlugInDriverRef, _ deviceObjectID: AudioObjectID, _ changeAction: UInt64, _ changeInfo: UnsafeMutableRawPointer?) -> OSStatus {
    halLog("PerformDeviceConfigurationChange: device=\(deviceObjectID) action=\(changeAction)")
    return noErr
}

private func driverAbortDeviceConfigurationChange(_ driver: AudioServerPlugInDriverRef, _ deviceObjectID: AudioObjectID, _ changeAction: UInt64, _ changeInfo: UnsafeMutableRawPointer?) -> OSStatus {
    halLog("AbortDeviceConfigurationChange: device=\(deviceObjectID)")
    return noErr
}

// MARK: - HasProperty

private func driverHasProperty(_ driver: AudioServerPlugInDriverRef, _ objectID: AudioObjectID, _ clientPID: pid_t, _ address: UnsafePointer<AudioObjectPropertyAddress>) -> DarwinBoolean {
    let sel = address.pointee.mSelector

    // Common properties for all objects
    let commonProps: Set<UInt32> = [
        kSelector_BaseClass, kSelector_Class, kSelector_Owner,
        kSelector_Name, kSelector_Manufacturer, kSelector_OwnedObjects
    ]

    switch objectID {
    case kPlugInObjectID:
        let pluginProps: Set<UInt32> = [
            kSelector_DeviceList, kSelector_ResourceBundle, kSelector_BundleID,
            kSelector_TranslateUID, kSelector_BoxList, kSelector_CustomPropertyInfo
        ]
        let hasIt = commonProps.contains(sel) || pluginProps.contains(sel)
        return DarwinBoolean(hasIt)

    case kDeviceObjectID:
        let deviceProps: Set<UInt32> = [
            kSelector_DeviceUID, kSelector_ModelUID, kSelector_TransportType,
            kSelector_RelatedDevices, kSelector_ClockDomain, kSelector_DeviceIsAlive,
            kSelector_DeviceIsRunning, kSelector_CanBeDefault, kSelector_CanBeSystemDefault,
            kSelector_Latency, kSelector_Streams, kSelector_SafetyOffset,
            kSelector_NominalSampleRate, kSelector_AvailableSampleRates, kSelector_Icon,
            kSelector_IsHidden, kSelector_PreferredStereo, kSelector_PreferredLayout,
            kSelector_ZeroTimePeriod, kSelector_ClockAlgorithm, kSelector_ClockIsStable,
            kSelector_BufferFrameSize, kSelector_BufferSizeRange, kSelector_StreamConfig,
            kSelector_ControlList, kSelector_ConfigApp, kSelector_Identify
        ]
        let hasIt = commonProps.contains(sel) || deviceProps.contains(sel)
        return DarwinBoolean(hasIt)

    case kInputStreamObjectID, kOutputStreamObjectID:
        let streamProps: Set<UInt32> = [
            kSelector_StreamIsActive, kSelector_StreamDirection, kSelector_TerminalType,
            kSelector_StartingChannel, kSelector_Latency, kSelector_VirtualFormat,
            kSelector_AvailableVirtualFmts, kSelector_PhysicalFormat, kSelector_AvailablePhysicalFmts
        ]
        let hasIt = commonProps.contains(sel) || streamProps.contains(sel)
        return DarwinBoolean(hasIt)

    default:
        return DarwinBoolean(false)
    }
}

// MARK: - IsPropertySettable

private func driverIsPropertySettable(_ driver: AudioServerPlugInDriverRef, _ objectID: AudioObjectID, _ clientPID: pid_t, _ address: UnsafePointer<AudioObjectPropertyAddress>, _ outIsSettable: UnsafeMutablePointer<DarwinBoolean>) -> OSStatus {
    let sel = address.pointee.mSelector

    let settableProps: Set<UInt32> = [
        kSelector_NominalSampleRate,
        kSelector_BufferFrameSize,
        kSelector_VirtualFormat,
        kSelector_PhysicalFormat
    ]

    outIsSettable.pointee = DarwinBoolean(settableProps.contains(sel))
    return noErr
}

// MARK: - GetPropertyDataSize

private func driverGetPropertyDataSize(_ driver: AudioServerPlugInDriverRef, _ objectID: AudioObjectID, _ clientPID: pid_t, _ address: UnsafePointer<AudioObjectPropertyAddress>, _ qualifierDataSize: UInt32, _ qualifierData: UnsafeRawPointer?, _ outDataSize: UnsafeMutablePointer<UInt32>) -> OSStatus {
    let sel = address.pointee.mSelector
    let scope = address.pointee.mScope

    // UInt32 properties
    let uint32Props: Set<UInt32> = [
        kSelector_BaseClass, kSelector_Class, kSelector_Owner, kSelector_TransportType,
        kSelector_ClockDomain, kSelector_DeviceIsAlive, kSelector_DeviceIsRunning,
        kSelector_CanBeDefault, kSelector_CanBeSystemDefault, kSelector_Latency,
        kSelector_SafetyOffset, kSelector_IsHidden, kSelector_BufferFrameSize,
        kSelector_ZeroTimePeriod, kSelector_ClockAlgorithm, kSelector_ClockIsStable,
        kSelector_StreamIsActive, kSelector_StreamDirection, kSelector_TerminalType,
        kSelector_StartingChannel, kSelector_Identify
    ]

    if uint32Props.contains(sel) {
        outDataSize.pointee = UInt32(MemoryLayout<UInt32>.size)
        return noErr
    }

    // Float64 properties
    if sel == kSelector_NominalSampleRate {
        outDataSize.pointee = UInt32(MemoryLayout<Float64>.size)
        return noErr
    }

    // CFString properties
    let cfstringProps: Set<UInt32> = [
        kSelector_Name, kSelector_Manufacturer, kSelector_DeviceUID, kSelector_ModelUID,
        kSelector_ResourceBundle, kSelector_BundleID, kSelector_SerialNumber,
        kSelector_FirmwareVersion, kSelector_ConfigApp
    ]

    if cfstringProps.contains(sel) {
        outDataSize.pointee = UInt32(MemoryLayout<CFString>.size)
        return noErr
    }

    // Special cases
    switch sel {
    case kSelector_DeviceList:
        outDataSize.pointee = UInt32(MemoryLayout<AudioObjectID>.size)  // 1 device

    case kSelector_OwnedObjects:
        switch objectID {
        case kPlugInObjectID:
            outDataSize.pointee = UInt32(MemoryLayout<AudioObjectID>.size)  // 1 device
        case kDeviceObjectID:
            outDataSize.pointee = UInt32(MemoryLayout<AudioObjectID>.size)  // 1 stream (output only)
        default:
            outDataSize.pointee = 0
        }

    case kSelector_Streams:
        // Output-only device: no input streams
        if scope == kScope_Input {
            outDataSize.pointee = 0
        } else {
            outDataSize.pointee = UInt32(MemoryLayout<AudioObjectID>.size)  // 1 output stream
        }

    case kSelector_RelatedDevices:
        outDataSize.pointee = UInt32(MemoryLayout<AudioObjectID>.size)

    case kSelector_AvailableSampleRates:
        outDataSize.pointee = UInt32(MemoryLayout<AudioValueRange>.size * kSupportedSampleRates.count)

    case kSelector_BufferSizeRange:
        outDataSize.pointee = UInt32(MemoryLayout<AudioValueRange>.size)

    case kSelector_VirtualFormat, kSelector_PhysicalFormat:
        outDataSize.pointee = UInt32(MemoryLayout<AudioStreamBasicDescription>.size)

    case kSelector_AvailableVirtualFmts, kSelector_AvailablePhysicalFmts:
        outDataSize.pointee = UInt32(MemoryLayout<AudioStreamRangedDescription>.size * kSupportedSampleRates.count)

    case kSelector_StreamConfig:
        outDataSize.pointee = UInt32(MemoryLayout<AudioBufferList>.size)

    case kSelector_ControlList, kSelector_CustomPropertyInfo, kSelector_BoxList, kSelector_TranslateUID:
        outDataSize.pointee = 0

    case kSelector_PreferredStereo:
        outDataSize.pointee = UInt32(MemoryLayout<UInt32>.size * 2)

    case kSelector_PreferredLayout, kSelector_Icon:
        outDataSize.pointee = 0  // Not implemented

    default:
        halLog("Unknown property size: '\(fourCC(sel))' (0x\(String(format: "%08X", sel))) for \(getObjectType(objectID))")
        return kAudioHardwareUnknownPropertyError
    }

    return noErr
}

// MARK: - GetPropertyData

private func driverGetPropertyData(_ driver: AudioServerPlugInDriverRef, _ objectID: AudioObjectID, _ clientPID: pid_t, _ address: UnsafePointer<AudioObjectPropertyAddress>, _ qualifierDataSize: UInt32, _ qualifierData: UnsafeRawPointer?, _ inDataSize: UInt32, _ outDataSize: UnsafeMutablePointer<UInt32>, _ outData: UnsafeMutableRawPointer) -> OSStatus {
    let sel = address.pointee.mSelector
    let scope = address.pointee.mScope
    let state = DriverState.shared

    switch sel {
    // Class IDs
    case kSelector_BaseClass:
        outData.storeBytes(of: kClassID_Object, as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_Class:
        let classID: UInt32
        switch objectID {
        case kPlugInObjectID: classID = kClassID_PlugIn
        case kDeviceObjectID: classID = kClassID_Device
        case kInputStreamObjectID, kOutputStreamObjectID: classID = kClassID_Stream
        default: classID = kClassID_Object
        }
        outData.storeBytes(of: classID, as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_Owner:
        let owner: AudioObjectID
        switch objectID {
        case kPlugInObjectID: owner = kAudioObjectUnknown
        case kDeviceObjectID: owner = kPlugInObjectID
        case kInputStreamObjectID, kOutputStreamObjectID: owner = kDeviceObjectID
        default: owner = kAudioObjectUnknown
        }
        outData.storeBytes(of: owner, as: AudioObjectID.self)
        outDataSize.pointee = 4

    // String properties
    case kSelector_Name:
        let name: String
        switch objectID {
        case kDeviceObjectID: name = "SotF Virtual Audio"
        case kInputStreamObjectID: name = "SotF Input"
        case kOutputStreamObjectID: name = "SotF Output"
        default: name = "SotF HAL"
        }
        let cfStr = createCFString(name)
        outData.storeBytes(of: cfStr.toOpaque(), as: UnsafeRawPointer.self)
        outDataSize.pointee = UInt32(MemoryLayout<CFString>.size)

    case kSelector_Manufacturer:
        let cfStr = createCFString("Spinorama")
        outData.storeBytes(of: cfStr.toOpaque(), as: UnsafeRawPointer.self)
        outDataSize.pointee = UInt32(MemoryLayout<CFString>.size)

    case kSelector_DeviceUID:
        let cfStr = createCFString("SotFVirtualAudioDevice_UID")
        outData.storeBytes(of: cfStr.toOpaque(), as: UnsafeRawPointer.self)
        outDataSize.pointee = UInt32(MemoryLayout<CFString>.size)

    case kSelector_ModelUID:
        let cfStr = createCFString("SotFVirtualAudioDevice_ModelUID")
        outData.storeBytes(of: cfStr.toOpaque(), as: UnsafeRawPointer.self)
        outDataSize.pointee = UInt32(MemoryLayout<CFString>.size)

    case kSelector_BundleID:
        let cfStr = createCFString("org.spinorama.sotf-hal")
        outData.storeBytes(of: cfStr.toOpaque(), as: UnsafeRawPointer.self)
        outDataSize.pointee = UInt32(MemoryLayout<CFString>.size)

    case kSelector_ResourceBundle, kSelector_SerialNumber, kSelector_FirmwareVersion, kSelector_ConfigApp:
        let cfStr = createCFString("")
        outData.storeBytes(of: cfStr.toOpaque(), as: UnsafeRawPointer.self)
        outDataSize.pointee = UInt32(MemoryLayout<CFString>.size)

    // Plugin properties
    case kSelector_DeviceList:
        outData.storeBytes(of: kDeviceObjectID, as: AudioObjectID.self)
        outDataSize.pointee = 4
        halLog("Returning device list: [\(kDeviceObjectID)]")

    // Owned objects
    case kSelector_OwnedObjects:
        switch objectID {
        case kPlugInObjectID:
            outData.storeBytes(of: kDeviceObjectID, as: AudioObjectID.self)
            outDataSize.pointee = 4
        case kDeviceObjectID:
            outData.storeBytes(of: kOutputStreamObjectID, as: AudioObjectID.self)
            outDataSize.pointee = 4
        default:
            outDataSize.pointee = 0
        }

    // Device properties
    case kSelector_TransportType:
        outData.storeBytes(of: kTransport_Virtual, as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_RelatedDevices:
        outData.storeBytes(of: kDeviceObjectID, as: AudioObjectID.self)
        outDataSize.pointee = 4

    case kSelector_ClockDomain:
        outData.storeBytes(of: UInt32(0), as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_ClockAlgorithm:
        // kAudioDeviceClockAlgorithmSimpleIIR = 1
        outData.storeBytes(of: UInt32(1), as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_DeviceIsAlive, kSelector_ClockIsStable:
        outData.storeBytes(of: UInt32(1), as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_DeviceIsRunning:
        outData.storeBytes(of: state.isRunning ? UInt32(1) : UInt32(0), as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_CanBeDefault, kSelector_CanBeSystemDefault:
        // Output-only device: cannot be default input device
        let canBe: UInt32 = (scope == kScope_Input) ? 0 : 1
        outData.storeBytes(of: canBe, as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_Latency:
        // Return latency based on object
        let latency: UInt32
        switch objectID {
        case kDeviceObjectID:
            latency = state.bufferFrameSize
        case kInputStreamObjectID, kOutputStreamObjectID:
            latency = 0
        default:
            latency = 0
        }
        outData.storeBytes(of: latency, as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_SafetyOffset:
        // Safety offset in frames
        outData.storeBytes(of: UInt32(0), as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_IsHidden, kSelector_Identify:
        outData.storeBytes(of: UInt32(0), as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_NominalSampleRate:
        outData.storeBytes(of: state.sampleRate, as: Float64.self)
        outDataSize.pointee = 8

    case kSelector_AvailableSampleRates:
        let ranges = outData.assumingMemoryBound(to: AudioValueRange.self)
        for (i, rate) in kSupportedSampleRates.enumerated() {
            ranges[i] = AudioValueRange(mMinimum: rate, mMaximum: rate)
        }
        outDataSize.pointee = UInt32(MemoryLayout<AudioValueRange>.size * kSupportedSampleRates.count)

    case kSelector_BufferFrameSize:
        outData.storeBytes(of: state.bufferFrameSize, as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_ZeroTimePeriod:
        outData.storeBytes(of: state.bufferFrameSize, as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_BufferSizeRange:
        outData.storeBytes(of: AudioValueRange(mMinimum: 64, mMaximum: 4096), as: AudioValueRange.self)
        outDataSize.pointee = UInt32(MemoryLayout<AudioValueRange>.size)

    case kSelector_Streams:
        // Output-only device: no input streams
        if scope == kScope_Input {
            outDataSize.pointee = 0
        } else {
            outData.storeBytes(of: kOutputStreamObjectID, as: AudioObjectID.self)
            outDataSize.pointee = 4
        }

    case kSelector_PreferredStereo:
        let channels = outData.assumingMemoryBound(to: UInt32.self)
        channels[0] = 1
        channels[1] = 2
        outDataSize.pointee = 8

    case kSelector_StreamConfig:
        if scope == kScope_Input {
            // No input streams
            var bufferList = AudioBufferList()
            bufferList.mNumberBuffers = 0
            outData.storeBytes(of: bufferList, as: AudioBufferList.self)
            outDataSize.pointee = UInt32(MemoryLayout<AudioBufferList>.size)
        } else {
            // Output: 1 interleaved buffer with proper channel count
            var bufferList = AudioBufferList()
            bufferList.mNumberBuffers = 1
            bufferList.mBuffers.mNumberChannels = state.channelCount
            bufferList.mBuffers.mDataByteSize = 0
            bufferList.mBuffers.mData = nil
            outData.storeBytes(of: bufferList, as: AudioBufferList.self)
            outDataSize.pointee = UInt32(MemoryLayout<AudioBufferList>.size)
        }

    // Stream properties
    case kSelector_StreamIsActive:
        outData.storeBytes(of: UInt32(1), as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_StreamDirection:
        // 0 = output (playback), 1 = input (recording)
        let direction: UInt32 = (objectID == kInputStreamObjectID) ? 1 : 0
        outData.storeBytes(of: direction, as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_TerminalType:
        let terminal: UInt32 = (objectID == kInputStreamObjectID) ? kTerminal_Microphone : kTerminal_Speaker
        outData.storeBytes(of: terminal, as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_StartingChannel:
        outData.storeBytes(of: UInt32(1), as: UInt32.self)
        outDataSize.pointee = 4

    case kSelector_VirtualFormat, kSelector_PhysicalFormat:
        var asbd = AudioStreamBasicDescription()
        asbd.mSampleRate = state.sampleRate
        asbd.mFormatID = kAudioFormatLinearPCM
        asbd.mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked
        asbd.mBytesPerPacket = UInt32(state.channelCount) * 4
        asbd.mFramesPerPacket = 1
        asbd.mBytesPerFrame = UInt32(state.channelCount) * 4
        asbd.mChannelsPerFrame = state.channelCount
        asbd.mBitsPerChannel = 32
        outData.storeBytes(of: asbd, as: AudioStreamBasicDescription.self)
        outDataSize.pointee = UInt32(MemoryLayout<AudioStreamBasicDescription>.size)

    case kSelector_AvailableVirtualFmts, kSelector_AvailablePhysicalFmts:
        let formats = outData.assumingMemoryBound(to: AudioStreamRangedDescription.self)
        for (i, rate) in kSupportedSampleRates.enumerated() {
            var asbd = AudioStreamBasicDescription()
            asbd.mSampleRate = rate
            asbd.mFormatID = kAudioFormatLinearPCM
            asbd.mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked
            asbd.mBytesPerPacket = UInt32(state.channelCount) * 4
            asbd.mFramesPerPacket = 1
            asbd.mBytesPerFrame = UInt32(state.channelCount) * 4
            asbd.mChannelsPerFrame = state.channelCount
            asbd.mBitsPerChannel = 32
            formats[i] = AudioStreamRangedDescription(
                mFormat: asbd,
                mSampleRateRange: AudioValueRange(mMinimum: rate, mMaximum: rate)
            )
        }
        outDataSize.pointee = UInt32(MemoryLayout<AudioStreamRangedDescription>.size * kSupportedSampleRates.count)

    // Empty lists
    case kSelector_ControlList, kSelector_CustomPropertyInfo, kSelector_BoxList, kSelector_TranslateUID, kSelector_PreferredLayout, kSelector_Icon:
        outDataSize.pointee = 0

    default:
        halLog("Unknown property get: '\(fourCC(sel))' (0x\(String(format: "%08X", sel))) for \(getObjectType(objectID))")
        return kAudioHardwareUnknownPropertyError
    }

    return noErr
}

// MARK: - SetPropertyData

private func driverSetPropertyData(_ driver: AudioServerPlugInDriverRef, _ objectID: AudioObjectID, _ clientPID: pid_t, _ address: UnsafePointer<AudioObjectPropertyAddress>, _ qualifierDataSize: UInt32, _ qualifierData: UnsafeRawPointer?, _ dataSize: UInt32, _ data: UnsafeRawPointer) -> OSStatus {
    let sel = address.pointee.mSelector
    let state = DriverState.shared

    switch sel {
    case kSelector_NominalSampleRate:
        let newRate = data.load(as: Float64.self)
        if kSupportedSampleRates.contains(newRate) {
            state.sampleRate = newRate
            state.clock.setSampleRate(newRate)
            state.sharedAudio.updateSampleRate(UInt32(newRate))
            halLog("Set sample rate: \(newRate)")

            // Notify host of property change
            notifyPropertyChanged(objectID: kDeviceObjectID, selector: kSelector_NominalSampleRate)
        } else {
            halLog("Rejected sample rate: \(newRate)")
            return kAudioDeviceUnsupportedFormatError
        }

    case kSelector_BufferFrameSize:
        let newSize = data.load(as: UInt32.self)
        if newSize >= 64 && newSize <= 4096 {
            state.bufferFrameSize = newSize
            state.resetBuffers()
            halLog("Set buffer size: \(newSize)")

            notifyPropertyChanged(objectID: kDeviceObjectID, selector: kSelector_BufferFrameSize)
        } else {
            halLog("Rejected buffer size: \(newSize)")
            return kAudioHardwareIllegalOperationError
        }

    case kSelector_VirtualFormat, kSelector_PhysicalFormat:
        let asbd = data.load(as: AudioStreamBasicDescription.self)
        if kSupportedSampleRates.contains(asbd.mSampleRate) {
            state.sampleRate = asbd.mSampleRate
            state.clock.setSampleRate(asbd.mSampleRate)
            halLog("Set format sample rate: \(asbd.mSampleRate)")
        }

    default:
        halLog("SetProperty ignored: '\(fourCC(sel))'")
    }

    return noErr
}

// MARK: - Property Change Notification

private func notifyPropertyChanged(objectID: AudioObjectID, selector: UInt32, scope: UInt32 = kScope_Global, element: UInt32 = 0) {
    guard let host = DriverState.shared.host else { return }

    var address = AudioObjectPropertyAddress(
        mSelector: selector,
        mScope: scope,
        mElement: element
    )

    // AudioServerPlugInHostRef is const AudioServerPlugInHostInterface*
    // So host.pointee gives us the AudioServerPlugInHostInterface directly
    let hostInterface = host.pointee
    _ = hostInterface.PropertiesChanged(host, objectID, 1, &address)
}

// MARK: - IO Operations

private func driverStartIO(_ driver: AudioServerPlugInDriverRef, _ deviceObjectID: AudioObjectID, _ clientID: UInt32) -> OSStatus {
    halLog("StartIO: device=\(deviceObjectID) client=\(clientID)")

    let state = DriverState.shared

    // Check if driver is being disposed to prevent race conditions
    if state.isDisposed {
        halLog("StartIO rejected: driver is being disposed")
        return kAudioHardwareIllegalOperationError
    }

    let newCount = state.incrementIOClientCount()

    if newCount == 1 {
        // First client - start the clock
        state.clock.start(sampleRate: state.sampleRate)
        state.inputRingBuffer?.reset()
        state.outputRingBuffer?.reset()
        state.sharedAudio.setActive(true)
        halLog("IO started, clock running")

        // Log SharedMemory state for debugging
        halLog("SharedMemory state: \(state.sharedAudio.connectionStateDebug)")

        // Log device configuration
        halLog("Device config: sampleRate=\(state.sampleRate), bufferFrameSize=\(state.bufferFrameSize), channels=\(state.channelCount)")
    }

    return noErr
}

private func driverStopIO(_ driver: AudioServerPlugInDriverRef, _ deviceObjectID: AudioObjectID, _ clientID: UInt32) -> OSStatus {
    halLog("StopIO: device=\(deviceObjectID) client=\(clientID)")

    let state = DriverState.shared
    let newCount = state.decrementIOClientCount()

    // Guard against underflow (shouldn't happen, but be safe)
    if newCount < 0 {
        halLog("StopIO warning: ioClientCount underflow, resetting to 0")
        state.resetIOClientCount()
    }

    if newCount <= 0 {
        state.clock.stop()
        state.sharedAudio.setActive(false)
        halLog("IO stopped")
    }

    return noErr
}

private func driverGetZeroTimeStamp(_ driver: AudioServerPlugInDriverRef, _ deviceObjectID: AudioObjectID, _ clientID: UInt32, _ outSampleTime: UnsafeMutablePointer<Float64>, _ outHostTime: UnsafeMutablePointer<UInt64>, _ outSeed: UnsafeMutablePointer<UInt64>) -> OSStatus {
    let state = DriverState.shared
    let (sampleTime, hostTime, seed) = state.clock.getZeroTimeStamp(bufferFrameSize: state.bufferFrameSize)

    outSampleTime.pointee = sampleTime
    outHostTime.pointee = hostTime
    outSeed.pointee = seed

    // Log periodically (every ~1000 calls to avoid spam)
    struct ZeroTimeLogger {
        static var callCount: UInt64 = 0
    }
    ZeroTimeLogger.callCount += 1
    if ZeroTimeLogger.callCount == 1 || ZeroTimeLogger.callCount % 1000 == 0 {
        halLog("GetZeroTimeStamp[#\(ZeroTimeLogger.callCount)]: sampleTime=\(sampleTime), hostTime=\(hostTime), seed=\(seed)")
    }

    return noErr
}

private func ioOperationName(_ operationID: UInt32) -> String {
    switch operationID {
    case kIOOperation_Thread: return "Thread"
    case kIOOperation_Cycle: return "Cycle"
    case kIOOperation_ReadInput: return "ReadInput"
    case kIOOperation_ConvertInput: return "ConvertInput"
    case kIOOperation_ProcessInput: return "ProcessInput"
    case kIOOperation_ProcessOutput: return "ProcessOutput"
    case kIOOperation_MixOutput: return "MixOutput"
    case kIOOperation_ProcessMix: return "ProcessMix"
    case kIOOperation_ConvertMix: return "ConvertMix"
    case kIOOperation_WriteMix: return "WriteMix"
    default: return "Unknown"
    }
}

private func driverWillDoIOOperation(_ driver: AudioServerPlugInDriverRef, _ deviceObjectID: AudioObjectID, _ clientID: UInt32, _ operationID: UInt32, _ outWillDo: UnsafeMutablePointer<DarwinBoolean>, _ outWillDoInPlace: UnsafeMutablePointer<DarwinBoolean>) -> OSStatus {
    // We support:
    // - Cycle: IO timing notifications
    // - WriteMix: Receive audio from playback clients
    let willDo = (operationID == kIOOperation_Cycle ||
                  operationID == kIOOperation_WriteMix)
    outWillDo.pointee = DarwinBoolean(willDo)
    outWillDoInPlace.pointee = DarwinBoolean(true)

    // ALWAYS log every call for debugging
    let opName = ioOperationName(operationID)
    halLog("WillDoIOOperation: op=\(opName) (0x\(String(format: "%08X", operationID)) '\(fourCC(operationID))'), willDo=\(willDo)")

    return noErr
}

private func driverBeginIOOperation(_ driver: AudioServerPlugInDriverRef, _ deviceObjectID: AudioObjectID, _ clientID: UInt32, _ operationID: UInt32, _ ioBufferFrameSize: UInt32, _ ioCycleInfo: UnsafePointer<AudioServerPlugInIOCycleInfo>) -> OSStatus {
    return noErr
}

private func driverDoIOOperation(_ driver: AudioServerPlugInDriverRef, _ deviceObjectID: AudioObjectID, _ streamObjectID: AudioObjectID, _ clientID: UInt32, _ operationID: UInt32, _ ioBufferFrameSize: UInt32, _ ioCycleInfo: UnsafePointer<AudioServerPlugInIOCycleInfo>, _ ioMainBuffer: UnsafeMutableRawPointer?, _ ioSecondaryBuffer: UnsafeMutableRawPointer?) -> OSStatus {

    // Log first call for each operation type
    struct DoIOLogger {
        static var loggedOps: Set<UInt32> = []
    }
    if !DoIOLogger.loggedOps.contains(operationID) {
        DoIOLogger.loggedOps.insert(operationID)
        halLog("DoIOOperation: FIRST CALL for op=\(ioOperationName(operationID)), stream=\(streamObjectID), frames=\(ioBufferFrameSize)")
    }

    // Handle Cycle operation (no buffer needed)
    if operationID == kIOOperation_Cycle {
        // Cycle operations notify us about IO timing - no action needed
        return noErr
    }

    guard let buffer = ioMainBuffer else { return noErr }

    let state = DriverState.shared
    let frameCount = Int(ioBufferFrameSize)
    let channelCount = Int(state.channelCount)
    let sampleCount = frameCount * channelCount
    let floatBuffer = buffer.assumingMemoryBound(to: Float.self)

    switch operationID {
    case kIOOperation_ReadInput:
        // Provide audio to clients (recording from virtual device)
        if state.sharedAudio.isConnected && state.sharedAudio.engineReady {
            // Read processed audio from Rust engine (this goes to hardware output)
            let framesRead = state.sharedAudio.readAudio(floatBuffer, frameCount: frameCount, channelCount: channelCount)
            // TRACE: Log frames consumed from shared memory and sent to hardware
            if framesRead > 0 {
                // Use os_log for real-time safe logging (halLog uses NSLog which can block)
                os_log("[AUDIO FLOW] HAL ReadInput: %d frames from shm -> hw output", log: logger, type: .debug, framesRead)
            }
        } else if state.loopbackEnabled, let outputBuffer = state.outputRingBuffer {
            // Loopback mode: return what was written to output
            _ = outputBuffer.readInterleaved(floatBuffer, frameCount: frameCount)
        } else {
            // No source - provide silence
            memset(floatBuffer, 0, sampleCount * MemoryLayout<Float>.size)
        }

    case kIOOperation_WriteMix:
        // Receive audio from clients (playback to virtual device)
        // Always store in loopback buffer first (ensures audio flows even without daemon)
        if state.loopbackEnabled, let outputBuffer = state.outputRingBuffer {
            _ = outputBuffer.writeInterleaved(floatBuffer, frameCount: frameCount)
        }

        // Periodic diagnostic logging (every ~2 seconds at 48kHz with 512 frame buffers)
        // Use a simple counter to avoid logging every frame
        struct DiagCounter {
            static var count: UInt64 = 0
            static var firstCallLogged: Bool = false
        }
        DiagCounter.count += 1

        // Log on very first call to confirm WriteMix is being invoked
        if !DiagCounter.firstCallLogged {
            DiagCounter.firstCallLogged = true
            halLog("WriteMix: FIRST CALL - frameCount=\(frameCount), channels=\(channelCount), sampleCount=\(sampleCount)")
        }

        let shouldLogDiag = (DiagCounter.count % 200) == 0

        let isConnected = state.sharedAudio.isConnected
        let engineReady = state.sharedAudio.engineReady

        // Compute RMS and peak of the incoming CoreAudio buffer to verify data is non-zero
        if shouldLogDiag {
            var mainRms: Float = 0.0
            var mainPeak: Float = 0.0
            let checkCount = min(sampleCount, 1024)
            for i in 0..<checkCount {
                let sample = floatBuffer[i]
                mainRms += sample * sample
                let absSample = abs(sample)
                if absSample > mainPeak { mainPeak = absSample }
            }
            mainRms = sqrtf(mainRms / Float(checkCount))

            // Also check ioSecondaryBuffer - CoreAudio may place mixed audio there
            var secRms: Float = 0.0
            var secPeak: Float = 0.0
            if let secBuf = ioSecondaryBuffer {
                let secFloat = secBuf.assumingMemoryBound(to: Float.self)
                for i in 0..<checkCount {
                    let sample = secFloat[i]
                    secRms += sample * sample
                    let absSample = abs(sample)
                    if absSample > secPeak { secPeak = absSample }
                }
                secRms = sqrtf(secRms / Float(checkCount))
            }

            os_log("[DIAG] WriteMix: conn=%{public}d eng=%{public}d mainRMS=%{public}.6f mainPeak=%{public}.6f secRMS=%{public}.6f secPeak=%{public}.6f frames=%{public}d sec=%{public}d",
                   log: logger, type: .error,
                   isConnected ? 1 : 0,
                   engineReady ? 1 : 0,
                   mainRms, mainPeak, secRms, secPeak, frameCount,
                   ioSecondaryBuffer != nil ? 1 : 0)
        }

        // Also send to Rust engine if connected and ready
        if isConnected && engineReady {
            let framesWritten = state.sharedAudio.writeAudio(floatBuffer, frameCount: frameCount, channelCount: channelCount)
            // TRACE: Log frames received from macOS apps and written to shared memory
            if framesWritten > 0 {
                if shouldLogDiag {
                    os_log("[AUDIO FLOW] HAL WriteMix: %d frames from app -> shm", log: logger, type: .error, framesWritten)
                }
            } else if framesWritten == 0 {
                os_log("[AUDIO FLOW] HAL WriteMix: buffer full, dropped %d frames", log: logger, type: .error, frameCount)
            }
        } else if shouldLogDiag {
            // Log why we're not sending to daemon
            os_log("[AUDIO FLOW] HAL WriteMix: NOT sending to daemon (isConnected=%{public}d, engineReady=%{public}d)",
                   log: logger, type: .error,
                   isConnected ? 1 : 0,
                   engineReady ? 1 : 0)
        }

    default:
        break
    }

    return noErr
}

private func driverEndIOOperation(_ driver: AudioServerPlugInDriverRef, _ deviceObjectID: AudioObjectID, _ clientID: UInt32, _ operationID: UInt32, _ ioBufferFrameSize: UInt32, _ ioCycleInfo: UnsafePointer<AudioServerPlugInIOCycleInfo>) -> OSStatus {
    return noErr
}

// MARK: - COM Interface

// Reference count using atomic operations for thread safety
// Multiple CoreAudio clients can call AddRef/Release concurrently
private var gRefCount: Int32 = 1

private func queryInterface(_ self_: UnsafeMutableRawPointer?, _ iid: REFIID, _ ppv: UnsafeMutablePointer<LPVOID?>?) -> HRESULT {
    halLog("QueryInterface")
    guard let ppv = ppv else { return -2147467261 }  // E_POINTER
    ppv.pointee = self_
    OSAtomicIncrement32(&gRefCount)
    return 0  // S_OK
}

private func addRef(_ self_: UnsafeMutableRawPointer?) -> ULONG {
    let newCount = OSAtomicIncrement32(&gRefCount)
    return ULONG(newCount)
}

private func release(_ self_: UnsafeMutableRawPointer?) -> ULONG {
    let newCount = OSAtomicDecrement32(&gRefCount)
    return ULONG(max(0, newCount))
}

// MARK: - Driver Interface

private var gDriverInterface = AudioServerPlugInDriverInterface(
    _reserved: nil,
    QueryInterface: { s, i, p in queryInterface(s, i, p) },
    AddRef: { s in addRef(s) },
    Release: { s in release(s) },
    Initialize: { d, h in driverInitialize(d, h) },
    CreateDevice: { d, desc, c, id in driverCreateDevice(d, desc, c, id) },
    DestroyDevice: { d, id in driverDestroyDevice(d, id) },
    AddDeviceClient: { d, id, c in driverAddDeviceClient(d, id, c) },
    RemoveDeviceClient: { d, id, c in driverRemoveDeviceClient(d, id, c) },
    PerformDeviceConfigurationChange: { d, id, a, i in driverPerformDeviceConfigurationChange(d, id, a, i) },
    AbortDeviceConfigurationChange: { d, id, a, i in driverAbortDeviceConfigurationChange(d, id, a, i) },
    HasProperty: { d, o, p, a in driverHasProperty(d, o, p, a) },
    IsPropertySettable: { d, o, p, a, s in driverIsPropertySettable(d, o, p, a, s) },
    GetPropertyDataSize: { d, o, p, a, qs, q, os in driverGetPropertyDataSize(d, o, p, a, qs, q, os) },
    GetPropertyData: { d, o, p, a, qs, q, is_, os, od in driverGetPropertyData(d, o, p, a, qs, q, is_, os, od) },
    SetPropertyData: { d, o, p, a, qs, q, s, dt in driverSetPropertyData(d, o, p, a, qs, q, s, dt) },
    StartIO: { d, id, c in driverStartIO(d, id, c) },
    StopIO: { d, id, c in driverStopIO(d, id, c) },
    GetZeroTimeStamp: { d, id, c, st, ht, sd in driverGetZeroTimeStamp(d, id, c, st, ht, sd) },
    WillDoIOOperation: { d, id, c, op, w, ip in driverWillDoIOOperation(d, id, c, op, w, ip) },
    BeginIOOperation: { d, id, c, op, bs, ci in driverBeginIOOperation(d, id, c, op, bs, ci) },
    DoIOOperation: { d, id, sid, c, op, bs, ci, mb, sb in driverDoIOOperation(d, id, sid, c, op, bs, ci, mb, sb) },
    EndIOOperation: { d, id, c, op, bs, ci in driverEndIOOperation(d, id, c, op, bs, ci) }
)

// Stable global pointer to the interface struct
private var gDriverInterfacePtr: UnsafeMutablePointer<AudioServerPlugInDriverInterface>? = nil

// Stable global "pointer to pointer" - this is what CoreAudio expects as AudioServerPlugInDriverRef
// AudioServerPlugInDriverRef = const AudioServerPlugInDriverInterface**
// We need a stable memory location that contains the pointer to the interface
private var gDriverRef: UnsafeMutablePointer<UnsafeMutablePointer<AudioServerPlugInDriverInterface>?>? = nil

// MARK: - Factory Function

@_cdecl("SotFHALDriverFactory")
public func SotFHALDriverFactory(_ allocator: CFAllocator?, _ requestedTypeUUID: CFUUID?) -> UnsafeMutableRawPointer? {
    halLog("Factory called")

    // kAudioServerPlugInTypeUUID = 443ABAB8-E7B3-491A-B985-BEB9187030DB
    let expectedUUID = CFUUIDCreateFromString(nil, "443ABAB8-E7B3-491A-B985-BEB9187030DB" as CFString)
    guard let requestedTypeUUID = requestedTypeUUID, CFEqual(requestedTypeUUID, expectedUUID) else {
        halLog("Factory: wrong UUID")
        return nil
    }

    if gDriverRef == nil {
        // Allocate the interface struct
        gDriverInterfacePtr = UnsafeMutablePointer<AudioServerPlugInDriverInterface>.allocate(capacity: 1)
        gDriverInterfacePtr!.pointee = gDriverInterface

        // Allocate the pointer-to-pointer (AudioServerPlugInDriverRef)
        // This must be a stable memory location that persists for the lifetime of the driver
        gDriverRef = UnsafeMutablePointer<UnsafeMutablePointer<AudioServerPlugInDriverInterface>?>.allocate(capacity: 1)
        gDriverRef!.pointee = gDriverInterfacePtr
    }

    halLog("Factory returning interface at \(String(describing: gDriverRef))")
    // Return the stable pointer-to-pointer (not &gDriverInterfacePtr which is temporary!)
    return UnsafeMutableRawPointer(gDriverRef!)
}
