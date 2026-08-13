// SpeechDenoiserAudioUnit.swift
// SOTF Speech Denoiser Audio Unit

import AVFoundation

public class SpeechDenoiserAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "SpeechDenoiser" }
    override public class var pluginSubtype: String { "SOSd" }
    override public class var pluginName: String { "SOTF: Speech Denoiser" }

    public override var channelCapabilities: [NSNumber]? {
        [NSNumber(value: 1), NSNumber(value: 1),
         NSNumber(value: 2), NSNumber(value: 2)]
    }

    public override func allocateRenderResources() throws {
        let inputFormat = inputBusses[0].format
        let outputFormat = outputBusses[0].format
        let supportedChannels = (1...2).contains(Int(inputFormat.channelCount))
            && inputFormat.channelCount == outputFormat.channelCount
        let supportedRate = inputFormat.sampleRate == 48_000
            && outputFormat.sampleRate == 48_000
        guard supportedChannels && supportedRate else {
            throw NSError(
                domain: NSOSStatusErrorDomain,
                code: Int(kAudioUnitErr_FormatNotSupported)
            )
        }
        try super.allocateRenderResources()
    }
}
