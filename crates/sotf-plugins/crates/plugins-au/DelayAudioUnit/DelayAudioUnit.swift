// DelayAudioUnit.swift
// SOTF Delay Audio Unit

import AVFoundation

public class DelayAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Delay" }
    override public class var pluginSubtype: String { "SODY" }
    override public class var pluginName: String { "SOTF: Delay" }
}
