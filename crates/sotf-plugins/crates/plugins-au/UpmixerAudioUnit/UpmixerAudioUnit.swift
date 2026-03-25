// UpmixerAudioUnit.swift
// SOTF Upmixer Audio Unit

import AVFoundation

public class UpmixerAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Upmixer" }
    override public class var pluginSubtype: String { "SOUp" }
    override public class var pluginName: String { "SOTF: Upmixer" }
}
