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
#include <mach/mach_time.h>
#include <pthread.h>
#include <dispatch/dispatch.h>

// Object IDs - must be unique within the driver
enum {
    kObjectID_PlugIn                = 1,
    kObjectID_Device                = 2,
    kObjectID_Stream_Input          = 3,
    kObjectID_Stream_Output         = 4,
};

// Driver configuration
#define kDevice_Name                "SotF Virtual Device"
#define kDevice_Manufacturer        "Spinorama"
#define kDevice_UID                 "SotF_VirtualDevice_UID"
#define kDevice_ModelUID            "SotF_Model_UID"
#define kDevice_SampleRate          48000.0
#define kDevice_ChannelCount        2
#define kDevice_BufferFrameSize     512

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
    AudioServerPlugInDriverInterface** interfacePtr;
    CFUUIDRef factoryID;
    UInt32 refCount;
    AudioServerPlugInHostRef host;

    // IO state
    Boolean isRunning;
    UInt64 anchorHostTime;
    Float64 anchorSampleTime;
    UInt64 timestampSeed;

    pthread_mutex_t mutex;
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
                gDriver->interfacePtr = &gDriver->interface;
                gDriver->factoryID = CFUUIDCreateFromString(NULL, CFSTR("5A4E28B8-93F4-4B8A-B5E2-3D9F6A8C7E01"));
                CFPlugInAddInstanceForFactory(gDriver->factoryID);
                gDriver->refCount = 1;
                gDriver->host = NULL;
                gDriver->isRunning = false;
                gDriver->anchorHostTime = 0;
                gDriver->anchorSampleTime = 0;
                gDriver->timestampSeed = 1;
                pthread_mutex_init(&gDriver->mutex, NULL);

                fprintf(stderr, "AudioHALDriver: Driver instance created\n");
            }
        } else {
            AddRef(gDriver);
        }
        return &gDriver->interfacePtr;
    }

    return NULL;
}

#pragma mark - COM Interface

static HRESULT QueryInterface(void* inDriver, REFIID inUUID, LPVOID* outInterface) {
    CFUUIDRef requestedUUID = CFUUIDCreateFromUUIDBytes(NULL, inUUID);
    CFUUIDRef unknownUUID = CFUUIDGetConstantUUIDWithBytes(NULL, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46);
    CFUUIDRef driverUUID = kAudioServerPlugInDriverInterfaceUUID;

    if (CFEqual(requestedUUID, unknownUUID) || CFEqual(requestedUUID, driverUUID)) {
        AddRef(inDriver);
        *outInterface = &gDriver->interfacePtr;
        CFRelease(requestedUUID);
        return S_OK;
    }

    CFRelease(requestedUUID);
    *outInterface = NULL;
    return E_NOINTERFACE;
}

static ULONG AddRef(void* inDriver) {
    if (gDriver != NULL) {
        gDriver->refCount++;
        return gDriver->refCount;
    }
    return 0;
}

static ULONG Release(void* inDriver) {
    if (gDriver != NULL) {
        gDriver->refCount--;
        if (gDriver->refCount == 0) {
            pthread_mutex_destroy(&gDriver->mutex);
            CFPlugInRemoveInstanceForFactory(gDriver->factoryID);
            CFRelease(gDriver->factoryID);
            free(gDriver);
            gDriver = NULL;
            return 0;
        }
        return gDriver->refCount;
    }
    return 0;
}

#pragma mark - Driver Operations

static OSStatus Initialize(AudioServerPlugInDriverRef inDriver, AudioServerPlugInHostRef inHost) {
    fprintf(stderr, "AudioHALDriver: Initialize called\n");

    if (gDriver == NULL) {
        return kAudioHardwareUnspecifiedError;
    }

    gDriver->host = inHost;
    fprintf(stderr, "AudioHALDriver: Initialized successfully\n");
    return kAudioHardwareNoError;
}

static OSStatus CreateDevice(AudioServerPlugInDriverRef inDriver, CFDictionaryRef inDescription, const AudioServerPlugInClientInfo* inClientInfo, AudioObjectID* outDeviceObjectID) {
    *outDeviceObjectID = kObjectID_Device;
    return kAudioHardwareNoError;
}

static OSStatus DestroyDevice(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID) {
    return kAudioHardwareNoError;
}

static OSStatus AddDeviceClient(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, const AudioServerPlugInClientInfo* inClientInfo) {
    return kAudioHardwareNoError;
}

static OSStatus RemoveDeviceClient(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, const AudioServerPlugInClientInfo* inClientInfo) {
    return kAudioHardwareNoError;
}

static OSStatus PerformDeviceConfigurationChange(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt64 inChangeAction, void* inChangeInfo) {
    return kAudioHardwareNoError;
}

static OSStatus AbortDeviceConfigurationChange(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt64 inChangeAction, void* inChangeInfo) {
    return kAudioHardwareNoError;
}

#pragma mark - Property Operations

static Boolean HasProperty(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress) {
    switch (inObjectID) {
        case kObjectID_PlugIn:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                case kAudioObjectPropertyClass:
                case kAudioObjectPropertyOwner:
                case kAudioObjectPropertyManufacturer:
                case kAudioObjectPropertyOwnedObjects:
                case kAudioPlugInPropertyDeviceList:
                case kAudioPlugInPropertyTranslateUIDToDevice:
                case kAudioPlugInPropertyResourceBundle:
                    return true;
            }
            break;

        case kObjectID_Device:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                case kAudioObjectPropertyClass:
                case kAudioObjectPropertyOwner:
                case kAudioObjectPropertyName:
                case kAudioObjectPropertyManufacturer:
                case kAudioObjectPropertyOwnedObjects:
                case kAudioDevicePropertyDeviceUID:
                case kAudioDevicePropertyModelUID:
                case kAudioDevicePropertyTransportType:
                case kAudioDevicePropertyRelatedDevices:
                case kAudioDevicePropertyClockDomain:
                case kAudioDevicePropertyDeviceIsAlive:
                case kAudioDevicePropertyDeviceIsRunning:
                case kAudioDevicePropertyDeviceCanBeDefaultDevice:
                case kAudioDevicePropertyDeviceCanBeDefaultSystemDevice:
                case kAudioDevicePropertyLatency:
                case kAudioDevicePropertyStreams:
                case kAudioObjectPropertyControlList:
                case kAudioDevicePropertySafetyOffset:
                case kAudioDevicePropertyNominalSampleRate:
                case kAudioDevicePropertyAvailableNominalSampleRates:
                case kAudioDevicePropertyIsHidden:
                case kAudioDevicePropertyZeroTimeStampPeriod:
                case kAudioDevicePropertyIcon:
                    return true;
            }
            break;

        case kObjectID_Stream_Input:
        case kObjectID_Stream_Output:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                case kAudioObjectPropertyClass:
                case kAudioObjectPropertyOwner:
                case kAudioStreamPropertyIsActive:
                case kAudioStreamPropertyDirection:
                case kAudioStreamPropertyTerminalType:
                case kAudioStreamPropertyStartingChannel:
                case kAudioStreamPropertyLatency:
                case kAudioStreamPropertyVirtualFormat:
                case kAudioStreamPropertyPhysicalFormat:
                case kAudioStreamPropertyAvailableVirtualFormats:
                case kAudioStreamPropertyAvailablePhysicalFormats:
                    return true;
            }
            break;
    }

    return false;
}

static OSStatus IsPropertySettable(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, Boolean* outIsSettable) {
    *outIsSettable = false;
    return kAudioHardwareNoError;
}

static OSStatus GetPropertyDataSize(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32* outDataSize) {
    OSStatus result = kAudioHardwareNoError;

    switch (inObjectID) {
        case kObjectID_PlugIn:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                case kAudioObjectPropertyClass:
                    *outDataSize = sizeof(AudioClassID);
                    break;
                case kAudioObjectPropertyOwner:
                    *outDataSize = sizeof(AudioObjectID);
                    break;
                case kAudioObjectPropertyManufacturer:
                    *outDataSize = sizeof(CFStringRef);
                    break;
                case kAudioObjectPropertyOwnedObjects:
                case kAudioPlugInPropertyDeviceList:
                    *outDataSize = sizeof(AudioObjectID);
                    break;
                case kAudioPlugInPropertyTranslateUIDToDevice:
                    *outDataSize = sizeof(AudioObjectID);
                    break;
                case kAudioPlugInPropertyResourceBundle:
                    *outDataSize = sizeof(CFStringRef);
                    break;
                default:
                    result = kAudioHardwareUnknownPropertyError;
                    break;
            }
            break;

        case kObjectID_Device:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                case kAudioObjectPropertyClass:
                    *outDataSize = sizeof(AudioClassID);
                    break;
                case kAudioObjectPropertyOwner:
                    *outDataSize = sizeof(AudioObjectID);
                    break;
                case kAudioObjectPropertyName:
                case kAudioObjectPropertyManufacturer:
                case kAudioDevicePropertyDeviceUID:
                case kAudioDevicePropertyModelUID:
                    *outDataSize = sizeof(CFStringRef);
                    break;
                case kAudioDevicePropertyTransportType:
                case kAudioDevicePropertyClockDomain:
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioDevicePropertyRelatedDevices:
                    *outDataSize = sizeof(AudioObjectID);
                    break;
                case kAudioDevicePropertyDeviceIsAlive:
                case kAudioDevicePropertyDeviceIsRunning:
                case kAudioDevicePropertyDeviceCanBeDefaultDevice:
                case kAudioDevicePropertyDeviceCanBeDefaultSystemDevice:
                case kAudioDevicePropertyIsHidden:
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioDevicePropertyLatency:
                case kAudioDevicePropertySafetyOffset:
                case kAudioDevicePropertyZeroTimeStampPeriod:
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioDevicePropertyStreams:
                    if (inAddress->mScope == kAudioObjectPropertyScopeInput) {
                        *outDataSize = sizeof(AudioObjectID);
                    } else if (inAddress->mScope == kAudioObjectPropertyScopeOutput) {
                        *outDataSize = sizeof(AudioObjectID);
                    } else {
                        *outDataSize = sizeof(AudioObjectID) * 2;
                    }
                    break;
                case kAudioObjectPropertyControlList:
                    *outDataSize = 0;
                    break;
                case kAudioObjectPropertyOwnedObjects:
                    *outDataSize = sizeof(AudioObjectID) * 2;
                    break;
                case kAudioDevicePropertyNominalSampleRate:
                    *outDataSize = sizeof(Float64);
                    break;
                case kAudioDevicePropertyAvailableNominalSampleRates:
                    *outDataSize = sizeof(AudioValueRange);
                    break;
                case kAudioDevicePropertyIcon:
                    *outDataSize = sizeof(CFURLRef);
                    break;
                default:
                    result = kAudioHardwareUnknownPropertyError;
                    break;
            }
            break;

        case kObjectID_Stream_Input:
        case kObjectID_Stream_Output:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                case kAudioObjectPropertyClass:
                    *outDataSize = sizeof(AudioClassID);
                    break;
                case kAudioObjectPropertyOwner:
                    *outDataSize = sizeof(AudioObjectID);
                    break;
                case kAudioStreamPropertyIsActive:
                case kAudioStreamPropertyDirection:
                case kAudioStreamPropertyTerminalType:
                case kAudioStreamPropertyStartingChannel:
                case kAudioStreamPropertyLatency:
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioStreamPropertyVirtualFormat:
                case kAudioStreamPropertyPhysicalFormat:
                    *outDataSize = sizeof(AudioStreamBasicDescription);
                    break;
                case kAudioStreamPropertyAvailableVirtualFormats:
                case kAudioStreamPropertyAvailablePhysicalFormats:
                    *outDataSize = sizeof(AudioStreamRangedDescription);
                    break;
                default:
                    result = kAudioHardwareUnknownPropertyError;
                    break;
            }
            break;

        default:
            result = kAudioHardwareBadObjectError;
            break;
    }

    return result;
}

static OSStatus GetPropertyData(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32 inDataSize, UInt32* outDataSize, void* outData) {
    OSStatus result = kAudioHardwareNoError;

    switch (inObjectID) {
        case kObjectID_PlugIn:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                    *((AudioClassID*)outData) = kAudioObjectClassID;
                    *outDataSize = sizeof(AudioClassID);
                    break;
                case kAudioObjectPropertyClass:
                    *((AudioClassID*)outData) = kAudioPlugInClassID;
                    *outDataSize = sizeof(AudioClassID);
                    break;
                case kAudioObjectPropertyOwner:
                    *((AudioObjectID*)outData) = kAudioObjectUnknown;
                    *outDataSize = sizeof(AudioObjectID);
                    break;
                case kAudioObjectPropertyManufacturer:
                    *((CFStringRef*)outData) = CFSTR(kDevice_Manufacturer);
                    *outDataSize = sizeof(CFStringRef);
                    break;
                case kAudioObjectPropertyOwnedObjects:
                case kAudioPlugInPropertyDeviceList:
                    *((AudioObjectID*)outData) = kObjectID_Device;
                    *outDataSize = sizeof(AudioObjectID);
                    break;
                case kAudioPlugInPropertyTranslateUIDToDevice:
                    if (inQualifierDataSize == sizeof(CFStringRef)) {
                        CFStringRef uid = *((CFStringRef*)inQualifierData);
                        if (CFStringCompare(uid, CFSTR(kDevice_UID), 0) == kCFCompareEqualTo) {
                            *((AudioObjectID*)outData) = kObjectID_Device;
                        } else {
                            *((AudioObjectID*)outData) = kAudioObjectUnknown;
                        }
                    } else {
                        *((AudioObjectID*)outData) = kAudioObjectUnknown;
                    }
                    *outDataSize = sizeof(AudioObjectID);
                    break;
                case kAudioPlugInPropertyResourceBundle:
                    *((CFStringRef*)outData) = CFSTR("");
                    *outDataSize = sizeof(CFStringRef);
                    break;
                default:
                    result = kAudioHardwareUnknownPropertyError;
                    break;
            }
            break;

        case kObjectID_Device:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                    *((AudioClassID*)outData) = kAudioObjectClassID;
                    *outDataSize = sizeof(AudioClassID);
                    break;
                case kAudioObjectPropertyClass:
                    *((AudioClassID*)outData) = kAudioDeviceClassID;
                    *outDataSize = sizeof(AudioClassID);
                    break;
                case kAudioObjectPropertyOwner:
                    *((AudioObjectID*)outData) = kObjectID_PlugIn;
                    *outDataSize = sizeof(AudioObjectID);
                    break;
                case kAudioObjectPropertyName:
                    *((CFStringRef*)outData) = CFSTR(kDevice_Name);
                    *outDataSize = sizeof(CFStringRef);
                    break;
                case kAudioObjectPropertyManufacturer:
                    *((CFStringRef*)outData) = CFSTR(kDevice_Manufacturer);
                    *outDataSize = sizeof(CFStringRef);
                    break;
                case kAudioDevicePropertyDeviceUID:
                    *((CFStringRef*)outData) = CFSTR(kDevice_UID);
                    *outDataSize = sizeof(CFStringRef);
                    break;
                case kAudioDevicePropertyModelUID:
                    *((CFStringRef*)outData) = CFSTR(kDevice_ModelUID);
                    *outDataSize = sizeof(CFStringRef);
                    break;
                case kAudioDevicePropertyTransportType:
                    *((UInt32*)outData) = kAudioDeviceTransportTypeVirtual;
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioDevicePropertyClockDomain:
                    *((UInt32*)outData) = 0;
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioDevicePropertyRelatedDevices:
                    *((AudioObjectID*)outData) = kObjectID_Device;
                    *outDataSize = sizeof(AudioObjectID);
                    break;
                case kAudioDevicePropertyDeviceIsAlive:
                    *((UInt32*)outData) = 1;
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioDevicePropertyDeviceIsRunning:
                    pthread_mutex_lock(&gDriver->mutex);
                    *((UInt32*)outData) = gDriver->isRunning ? 1 : 0;
                    pthread_mutex_unlock(&gDriver->mutex);
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioDevicePropertyDeviceCanBeDefaultDevice:
                case kAudioDevicePropertyDeviceCanBeDefaultSystemDevice:
                    *((UInt32*)outData) = 1;
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioDevicePropertyIsHidden:
                    *((UInt32*)outData) = 0;
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioDevicePropertyLatency:
                    *((UInt32*)outData) = 0;
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioDevicePropertySafetyOffset:
                    *((UInt32*)outData) = 0;
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioDevicePropertyZeroTimeStampPeriod:
                    *((UInt32*)outData) = kDevice_BufferFrameSize;
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioDevicePropertyStreams:
                    if (inAddress->mScope == kAudioObjectPropertyScopeInput) {
                        *((AudioObjectID*)outData) = kObjectID_Stream_Input;
                        *outDataSize = sizeof(AudioObjectID);
                    } else if (inAddress->mScope == kAudioObjectPropertyScopeOutput) {
                        *((AudioObjectID*)outData) = kObjectID_Stream_Output;
                        *outDataSize = sizeof(AudioObjectID);
                    } else {
                        AudioObjectID* ids = (AudioObjectID*)outData;
                        ids[0] = kObjectID_Stream_Input;
                        ids[1] = kObjectID_Stream_Output;
                        *outDataSize = sizeof(AudioObjectID) * 2;
                    }
                    break;
                case kAudioObjectPropertyControlList:
                    *outDataSize = 0;
                    break;
                case kAudioObjectPropertyOwnedObjects:
                    {
                        AudioObjectID* ids = (AudioObjectID*)outData;
                        ids[0] = kObjectID_Stream_Input;
                        ids[1] = kObjectID_Stream_Output;
                        *outDataSize = sizeof(AudioObjectID) * 2;
                    }
                    break;
                case kAudioDevicePropertyNominalSampleRate:
                    *((Float64*)outData) = kDevice_SampleRate;
                    *outDataSize = sizeof(Float64);
                    break;
                case kAudioDevicePropertyAvailableNominalSampleRates:
                    {
                        AudioValueRange* range = (AudioValueRange*)outData;
                        range->mMinimum = kDevice_SampleRate;
                        range->mMaximum = kDevice_SampleRate;
                        *outDataSize = sizeof(AudioValueRange);
                    }
                    break;
                case kAudioDevicePropertyIcon:
                    *((CFURLRef*)outData) = NULL;
                    *outDataSize = sizeof(CFURLRef);
                    break;
                default:
                    result = kAudioHardwareUnknownPropertyError;
                    break;
            }
            break;

        case kObjectID_Stream_Input:
        case kObjectID_Stream_Output:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                    *((AudioClassID*)outData) = kAudioObjectClassID;
                    *outDataSize = sizeof(AudioClassID);
                    break;
                case kAudioObjectPropertyClass:
                    *((AudioClassID*)outData) = kAudioStreamClassID;
                    *outDataSize = sizeof(AudioClassID);
                    break;
                case kAudioObjectPropertyOwner:
                    *((AudioObjectID*)outData) = kObjectID_Device;
                    *outDataSize = sizeof(AudioObjectID);
                    break;
                case kAudioStreamPropertyIsActive:
                    *((UInt32*)outData) = 1;
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioStreamPropertyDirection:
                    *((UInt32*)outData) = (inObjectID == kObjectID_Stream_Input) ? 1 : 0;
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioStreamPropertyTerminalType:
                    *((UInt32*)outData) = (inObjectID == kObjectID_Stream_Input) ? kAudioStreamTerminalTypeMicrophone : kAudioStreamTerminalTypeSpeaker;
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioStreamPropertyStartingChannel:
                    *((UInt32*)outData) = 1;
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioStreamPropertyLatency:
                    *((UInt32*)outData) = 0;
                    *outDataSize = sizeof(UInt32);
                    break;
                case kAudioStreamPropertyVirtualFormat:
                case kAudioStreamPropertyPhysicalFormat:
                    {
                        AudioStreamBasicDescription* format = (AudioStreamBasicDescription*)outData;
                        format->mSampleRate = kDevice_SampleRate;
                        format->mFormatID = kAudioFormatLinearPCM;
                        format->mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagsNativeEndian | kAudioFormatFlagIsPacked;
                        format->mBytesPerPacket = sizeof(Float32) * kDevice_ChannelCount;
                        format->mFramesPerPacket = 1;
                        format->mBytesPerFrame = sizeof(Float32) * kDevice_ChannelCount;
                        format->mChannelsPerFrame = kDevice_ChannelCount;
                        format->mBitsPerChannel = sizeof(Float32) * 8;
                        *outDataSize = sizeof(AudioStreamBasicDescription);
                    }
                    break;
                case kAudioStreamPropertyAvailableVirtualFormats:
                case kAudioStreamPropertyAvailablePhysicalFormats:
                    {
                        AudioStreamRangedDescription* desc = (AudioStreamRangedDescription*)outData;
                        desc->mFormat.mSampleRate = kDevice_SampleRate;
                        desc->mFormat.mFormatID = kAudioFormatLinearPCM;
                        desc->mFormat.mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagsNativeEndian | kAudioFormatFlagIsPacked;
                        desc->mFormat.mBytesPerPacket = sizeof(Float32) * kDevice_ChannelCount;
                        desc->mFormat.mFramesPerPacket = 1;
                        desc->mFormat.mBytesPerFrame = sizeof(Float32) * kDevice_ChannelCount;
                        desc->mFormat.mChannelsPerFrame = kDevice_ChannelCount;
                        desc->mFormat.mBitsPerChannel = sizeof(Float32) * 8;
                        desc->mSampleRateRange.mMinimum = kDevice_SampleRate;
                        desc->mSampleRateRange.mMaximum = kDevice_SampleRate;
                        *outDataSize = sizeof(AudioStreamRangedDescription);
                    }
                    break;
                default:
                    result = kAudioHardwareUnknownPropertyError;
                    break;
            }
            break;

        default:
            result = kAudioHardwareBadObjectError;
            break;
    }

    return result;
}

static OSStatus SetPropertyData(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32 inDataSize, const void* inData) {
    return kAudioHardwareUnsupportedOperationError;
}

#pragma mark - IO Operations

static OSStatus StartIO(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID) {
    fprintf(stderr, "AudioHALDriver: StartIO called\n");

    pthread_mutex_lock(&gDriver->mutex);
    gDriver->isRunning = true;
    gDriver->anchorHostTime = mach_absolute_time();
    gDriver->anchorSampleTime = 0;
    gDriver->timestampSeed++;
    pthread_mutex_unlock(&gDriver->mutex);

    return kAudioHardwareNoError;
}

static OSStatus StopIO(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID) {
    fprintf(stderr, "AudioHALDriver: StopIO called\n");

    pthread_mutex_lock(&gDriver->mutex);
    gDriver->isRunning = false;
    pthread_mutex_unlock(&gDriver->mutex);

    return kAudioHardwareNoError;
}

static OSStatus GetZeroTimeStamp(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, Float64* outSampleTime, UInt64* outHostTime, UInt64* outSeed) {
    pthread_mutex_lock(&gDriver->mutex);

    // Get time info
    static mach_timebase_info_data_t timebaseInfo;
    if (timebaseInfo.denom == 0) {
        mach_timebase_info(&timebaseInfo);
    }

    UInt64 currentHostTime = mach_absolute_time();
    UInt64 hostTimeDelta = currentHostTime - gDriver->anchorHostTime;

    // Convert to nanoseconds
    Float64 hostTimeNanos = (Float64)hostTimeDelta * (Float64)timebaseInfo.numer / (Float64)timebaseInfo.denom;

    // Convert to samples
    Float64 sampleTime = (hostTimeNanos * kDevice_SampleRate) / 1000000000.0;

    // Align to buffer boundaries
    UInt64 bufferCount = (UInt64)(sampleTime / kDevice_BufferFrameSize);

    *outSampleTime = bufferCount * kDevice_BufferFrameSize;
    *outHostTime = gDriver->anchorHostTime + (UInt64)((*outSampleTime / kDevice_SampleRate) * 1000000000.0 * (Float64)timebaseInfo.denom / (Float64)timebaseInfo.numer);
    *outSeed = gDriver->timestampSeed;

    pthread_mutex_unlock(&gDriver->mutex);

    return kAudioHardwareNoError;
}

static OSStatus WillDoIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, Boolean* outWillDo, Boolean* outWillDoInPlace) {
    switch (inOperationID) {
        case kAudioServerPlugInIOOperationReadInput:
        case kAudioServerPlugInIOOperationWriteMix:
            *outWillDo = true;
            *outWillDoInPlace = true;
            break;
        default:
            *outWillDo = false;
            *outWillDoInPlace = true;
            break;
    }
    return kAudioHardwareNoError;
}

static OSStatus BeginIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo) {
    return kAudioHardwareNoError;
}

static OSStatus DoIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, AudioObjectID inStreamObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo, void* ioMainBuffer, void* ioSecondaryBuffer) {
    // For now, just pass silence
    if (ioMainBuffer != NULL) {
        size_t bufferSize = inIOBufferFrameSize * sizeof(Float32) * kDevice_ChannelCount;
        memset(ioMainBuffer, 0, bufferSize);
    }

    return kAudioHardwareNoError;
}

static OSStatus EndIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo) {
    return kAudioHardwareNoError;
}
