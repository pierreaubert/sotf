// DownmixAudioUnit.swift
// SOTF Downmix Audio Unit

import AVFoundation

public class DownmixAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Downmix" }
    override public class var pluginSubtype: String { "SODm" }
    override public class var pluginName: String { "SOTF: Downmix" }
    override public class var fixedOutputChannels: Int? { 2 }

    override public func pluginConfiguration(inputFormat: AVAudioFormat,
                                             outputFormat: AVAudioFormat) -> String {
        let channels = Int(inputFormat.channelCount)
        let tag = inputFormat.channelLayout?.layoutTag
        let layout: String?
        switch tag {
        case kAudioChannelLayoutTag_Atmos_5_1_2: layout = "5.1.2"
        case kAudioChannelLayoutTag_Atmos_5_1_4: layout = "5.1.4"
        case kAudioChannelLayoutTag_Atmos_7_1_2: layout = "7.1.2"
        case kAudioChannelLayoutTag_Atmos_7_1_4: layout = "7.1.4"
        case kAudioChannelLayoutTag_MPEG_7_1_C: layout = "7.1"
        default:
            layout = [1: "1.0", 2: "2.0", 3: "2.1", 5: "5.0", 6: "5.1",
                      12: "7.1.4", 14: "9.1.4", 16: "9.1.6"][channels]
        }
        if let layout = layout {
            return "{\"input_channels\":\(channels),\"input_layout\":\"\(layout)\"}"
        }
        // Ambiguous layouts without a Core Audio layout tag are rejected by
        // the Rust constructor rather than silently misrouted.
        return "{\"input_channels\":\(channels)}"
    }
}
