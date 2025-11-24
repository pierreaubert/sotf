/**
 * Core Audio HAL Driver Bridge
 *
 * This C implementation provides the entry point for macOS Core Audio to load
 * our audio driver. It implements the required CFPlugIn interface that Core Audio
 * expects from HAL driver bundles.
 */

#include <CoreAudio/AudioServerPlugIn.h>
#include <CoreFoundation/CoreFoundation.h>
#include <CoreFoundation/CFPlugInCOM.h>
#include <stdio.h>
#include <string.h>

// Additional Core Audio constants
#ifndef kAudioDevicePropertyStreamConfiguration
#define kAudioDevicePropertyStreamConfiguration 'slay'
#endif

// Driver identification
#define kAudioHALDriverFactoryUUID CFUUIDGetConstantUUIDWithBytes(NULL, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01)

// Forward declarations
static HRESULT QueryInterface(void* inDriver, REFIID inUUID, LPVOID* outInterface);
static ULONG AddRef(void* inDriver);
static ULONG Release(void* inDriver);
static OSStatus Initialize(AudioServerPlugInDriverRef inDriver, AudioServerPlugInHostRef inHost);
static OSStatus CreateDevice(AudioServerPlugInDriverRef inDriver, CFDictionaryRef inDescription, const AudioServerPlugInClientInfo* inClientInfo, AudioObjectID* outDeviceObjectID);
static OSStatus DestroyDevice(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID);
static OSStatus AddDeviceClient(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, const AudioServerPlugInClientInfo* inClientInfo);
static OSStatus RemoveDeviceClient(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, const AudioServerPlugInClientInfo* inClientInfo);
static OSStatus PerformDeviceConfigurationChange(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt64 inChangeAction, void* inChangeInfo);
static OSStatus AbortDeviceConfigurationChange(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt64 inChangeAction, void* inChangeInfo);
static Boolean HasProperty(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress);
static OSStatus IsPropertySettable(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, Boolean* outIsSettable);
static OSStatus GetPropertyDataSize(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32* outDataSize);
static OSStatus GetPropertyData(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32 inDataSize, UInt32* outDataSize, void* outData);
static OSStatus SetPropertyData(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32 inDataSize, const void* inData);
static OSStatus StartIO(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID);
static OSStatus StopIO(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID);
static OSStatus GetZeroTimeStamp(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, Float64* outSampleTime, UInt64* outHostTime, UInt64* outSeed);
static OSStatus WillDoIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, Boolean* outWillDo, Boolean* outWillDoInPlace);
static OSStatus BeginIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo);
static OSStatus DoIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, AudioObjectID inStreamObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo, void* ioMainBuffer, void* ioSecondaryBuffer);
static OSStatus EndIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo);

// Driver interface structure
static AudioServerPlugInDriverInterface gAudioServerPlugInDriverInterface = {
    NULL,
    QueryInterface,
    AddRef,
    Release,
    Initialize,
    CreateDevice,
    DestroyDevice,
    AddDeviceClient,
    RemoveDeviceClient,
    PerformDeviceConfigurationChange,
    AbortDeviceConfigurationChange,
    HasProperty,
    IsPropertySettable,
    GetPropertyDataSize,
    GetPropertyData,
    SetPropertyData,
    StartIO,
    StopIO,
    GetZeroTimeStamp,
    WillDoIOOperation,
    BeginIOOperation,
    DoIOOperation,
    EndIOOperation
};

// Driver instance structure
typedef struct {
    AudioServerPlugInDriverInterface* interface;
    CFUUIDRef factoryID;
    UInt32 refCount;
    AudioServerPlugInHostRef host;
    AudioObjectID deviceObjectID;
    UInt32 inputChannels;
    UInt32 outputChannels;
} AudioHALDriver;

static AudioHALDriver* gDriver = NULL;

#pragma mark - Factory Functions

void* AudioHALDriverFactory(CFAllocatorRef allocator, CFUUIDRef typeID) {
    fprintf(stderr, "AudioHALDriver: Factory called\n");

    if (CFEqual(typeID, kAudioServerPlugInTypeUUID)) {
        if (gDriver == NULL) {
            gDriver = (AudioHALDriver*)malloc(sizeof(AudioHALDriver));
            if (gDriver != NULL) {
                gDriver->interface = &gAudioServerPlugInDriverInterface;
                gDriver->factoryID = kAudioHALDriverFactoryUUID;
                CFPlugInAddInstanceForFactory(gDriver->factoryID);
                gDriver->refCount = 1;
                gDriver->host = NULL;
                gDriver->deviceObjectID = 0;
                gDriver->inputChannels = 16;   // Support up to 16 input channels
                gDriver->outputChannels = 16;  // Support up to 16 output channels

                fprintf(stderr, "AudioHALDriver: Driver instance created\n");
            }
        } else {
            AddRef(gDriver);
        }
        return gDriver;
    }

    return NULL;
}

#pragma mark - COM Interface

static HRESULT QueryInterface(void* inDriver, REFIID inUUID, LPVOID* outInterface) {
    AudioHALDriver* driver = (AudioHALDriver*)inDriver;

    // Create CFUUIDs from the UUIDs for comparison
    CFUUIDRef requestedUUID = CFUUIDCreateFromUUIDBytes(NULL, inUUID);
    CFUUIDRef unknownUUID = CFUUIDGetConstantUUIDWithBytes(NULL, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46);
    CFUUIDRef driverUUID = kAudioServerPlugInDriverInterfaceUUID;

    if (CFEqual(requestedUUID, unknownUUID) || CFEqual(requestedUUID, driverUUID)) {
        AddRef(driver);
        *outInterface = driver;
        CFRelease(requestedUUID);
        return S_OK;
    }

    CFRelease(requestedUUID);
    return E_NOINTERFACE;
}

static ULONG AddRef(void* inDriver) {
    AudioHALDriver* driver = (AudioHALDriver*)inDriver;
    driver->refCount++;
    return driver->refCount;
}

static ULONG Release(void* inDriver) {
    AudioHALDriver* driver = (AudioHALDriver*)inDriver;
    driver->refCount--;

    if (driver->refCount == 0) {
        CFPlugInRemoveInstanceForFactory(driver->factoryID);
        free(driver);
        gDriver = NULL;
        return 0;
    }

    return driver->refCount;
}

#pragma mark - Driver Operations

static OSStatus Initialize(AudioServerPlugInDriverRef inDriver, AudioServerPlugInHostRef inHost) {
    AudioHALDriver* driver = (AudioHALDriver*)inDriver;
    fprintf(stderr, "AudioHALDriver: Initialize called\n");

    driver->host = inHost;

    // For now, we'll create a simple virtual device
    // In a full implementation, this would initialize the Rust backend
    driver->deviceObjectID = 1; // Placeholder device ID

    fprintf(stderr, "AudioHALDriver: Initialized successfully\n");
    return kAudioHardwareNoError;
}

static OSStatus CreateDevice(AudioServerPlugInDriverRef inDriver, CFDictionaryRef inDescription, const AudioServerPlugInClientInfo* inClientInfo, AudioObjectID* outDeviceObjectID) {
    fprintf(stderr, "AudioHALDriver: CreateDevice called\n");
    *outDeviceObjectID = 0;
    return kAudioHardwareNoError;
}

static OSStatus DestroyDevice(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID) {
    fprintf(stderr, "AudioHALDriver: DestroyDevice called\n");
    return kAudioHardwareNoError;
}

static OSStatus AddDeviceClient(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, const AudioServerPlugInClientInfo* inClientInfo) {
    fprintf(stderr, "AudioHALDriver: AddDeviceClient called\n");
    return kAudioHardwareNoError;
}

static OSStatus RemoveDeviceClient(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, const AudioServerPlugInClientInfo* inClientInfo) {
    fprintf(stderr, "AudioHALDriver: RemoveDeviceClient called\n");
    return kAudioHardwareNoError;
}

static OSStatus PerformDeviceConfigurationChange(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt64 inChangeAction, void* inChangeInfo) {
    fprintf(stderr, "AudioHALDriver: PerformDeviceConfigurationChange called\n");
    return kAudioHardwareNoError;
}

static OSStatus AbortDeviceConfigurationChange(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt64 inChangeAction, void* inChangeInfo) {
    fprintf(stderr, "AudioHALDriver: AbortDeviceConfigurationChange called\n");
    return kAudioHardwareNoError;
}

#pragma mark - Property Operations

static Boolean HasProperty(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress) {
    // Simplified implementation - return true for basic properties
    switch (inAddress->mSelector) {
        case kAudioObjectPropertyName:
        case kAudioObjectPropertyManufacturer:
        case kAudioDevicePropertyDeviceUID:
        case kAudioDevicePropertyStreams:
        case kAudioDevicePropertyStreamConfiguration:
        case kAudioDevicePropertyNominalSampleRate:
        case kAudioDevicePropertyAvailableNominalSampleRates:
            return true;
        default:
            return false;
    }
}

static OSStatus IsPropertySettable(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, Boolean* outIsSettable) {
    *outIsSettable = false;
    return kAudioHardwareNoError;
}

static OSStatus GetPropertyDataSize(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32* outDataSize) {
    switch (inAddress->mSelector) {
        case kAudioObjectPropertyName:
        case kAudioObjectPropertyManufacturer:
        case kAudioDevicePropertyDeviceUID:
            *outDataSize = sizeof(CFStringRef);
            break;
        case kAudioDevicePropertyStreamConfiguration:
            *outDataSize = sizeof(AudioBufferList) + (15 * sizeof(AudioBuffer)); // Up to 16 channels
            break;
        case kAudioDevicePropertyNominalSampleRate:
            *outDataSize = sizeof(Float64);
            break;
        case kAudioDevicePropertyAvailableNominalSampleRates:
            *outDataSize = sizeof(AudioValueRange) * 3; // 44.1, 48, 96 kHz
            break;
        case kAudioDevicePropertyStreams:
            *outDataSize = 0; // No streams for now
            break;
        default:
            *outDataSize = 0;
            break;
    }
    return kAudioHardwareNoError;
}

static OSStatus GetPropertyData(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32 inDataSize, UInt32* outDataSize, void* outData) {
    AudioHALDriver* driver = (AudioHALDriver*)inDriver;

    switch (inAddress->mSelector) {
        case kAudioObjectPropertyName:
            if (inDataSize >= sizeof(CFStringRef)) {
                *(CFStringRef*)outData = CFSTR("SotF");
                *outDataSize = sizeof(CFStringRef);
            }
            break;
        case kAudioObjectPropertyManufacturer:
            if (inDataSize >= sizeof(CFStringRef)) {
                *(CFStringRef*)outData = CFSTR("Pierre F. Aubert");
                *outDataSize = sizeof(CFStringRef);
            }
            break;
        case kAudioDevicePropertyDeviceUID:
            if (inDataSize >= sizeof(CFStringRef)) {
                *(CFStringRef*)outData = CFSTR("SotF-001");
                *outDataSize = sizeof(CFStringRef);
            }
            break;
        case kAudioDevicePropertyStreamConfiguration: {
            if (inDataSize >= sizeof(AudioBufferList)) {
                AudioBufferList* bufferList = (AudioBufferList*)outData;
                UInt32 numChannels = 0;

                // Determine if this is input or output
                if (inAddress->mScope == kAudioObjectPropertyScopeInput) {
                    numChannels = driver->inputChannels;
                } else if (inAddress->mScope == kAudioObjectPropertyScopeOutput) {
                    numChannels = driver->outputChannels;
                }

                bufferList->mNumberBuffers = numChannels;
                for (UInt32 i = 0; i < numChannels; i++) {
                    bufferList->mBuffers[i].mNumberChannels = 1;
                    bufferList->mBuffers[i].mDataByteSize = 0;
                    bufferList->mBuffers[i].mData = NULL;
                }

                *outDataSize = sizeof(AudioBufferList) + ((numChannels - 1) * sizeof(AudioBuffer));
            }
            break;
        }
        case kAudioDevicePropertyNominalSampleRate:
            if (inDataSize >= sizeof(Float64)) {
                *(Float64*)outData = 44100.0;
                *outDataSize = sizeof(Float64);
            }
            break;
        case kAudioDevicePropertyAvailableNominalSampleRates: {
            if (inDataSize >= sizeof(AudioValueRange) * 3) {
                AudioValueRange* ranges = (AudioValueRange*)outData;
                ranges[0].mMinimum = 44100.0;
                ranges[0].mMaximum = 44100.0;
                ranges[1].mMinimum = 48000.0;
                ranges[1].mMaximum = 48000.0;
                ranges[2].mMinimum = 96000.0;
                ranges[2].mMaximum = 96000.0;
                *outDataSize = sizeof(AudioValueRange) * 3;
            }
            break;
        }
        default:
            *outDataSize = 0;
            break;
    }
    return kAudioHardwareNoError;
}

static OSStatus SetPropertyData(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32 inDataSize, const void* inData) {
    return kAudioHardwareUnsupportedOperationError;
}

#pragma mark - IO Operations

static OSStatus StartIO(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID) {
    fprintf(stderr, "AudioHALDriver: StartIO called\n");
    return kAudioHardwareNoError;
}

static OSStatus StopIO(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID) {
    fprintf(stderr, "AudioHALDriver: StopIO called\n");
    return kAudioHardwareNoError;
}

static OSStatus GetZeroTimeStamp(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, Float64* outSampleTime, UInt64* outHostTime, UInt64* outSeed) {
    *outSampleTime = 0.0;
    *outHostTime = 0;
    *outSeed = 0;
    return kAudioHardwareNoError;
}

static OSStatus WillDoIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, Boolean* outWillDo, Boolean* outWillDoInPlace) {
    *outWillDo = false;
    *outWillDoInPlace = false;
    return kAudioHardwareNoError;
}

static OSStatus BeginIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo) {
    return kAudioHardwareNoError;
}

static OSStatus DoIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, AudioObjectID inStreamObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo, void* ioMainBuffer, void* ioSecondaryBuffer) {
    AudioHALDriver* driver = (AudioHALDriver*)inDriver;

    // This is where audio processing would happen
    // For now, just pass through silence for all channels
    if (ioMainBuffer != NULL) {
        // Clear up to 16 output channels
        size_t bufferSize = inIOBufferFrameSize * sizeof(float) * driver->outputChannels;
        memset(ioMainBuffer, 0, bufferSize);
    }

    // TODO: Process input from ioMainBuffer (input channels) to output
    // TODO: Connect to Rust audio processing pipeline

    return kAudioHardwareNoError;
}

static OSStatus EndIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo) {
    return kAudioHardwareNoError;
}
