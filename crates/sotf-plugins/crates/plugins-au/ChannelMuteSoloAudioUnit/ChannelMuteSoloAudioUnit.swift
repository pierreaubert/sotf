// ChannelMuteSoloAudioUnit.swift
// SOTF Channel Mute/Solo Audio Unit

import AVFoundation

public class ChannelMuteSoloAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "ChannelMuteSolo" }
    override public class var pluginSubtype: String { "SOCs" }
    override public class var pluginName: String { "SOTF: Channel Mute/Solo" }
}
