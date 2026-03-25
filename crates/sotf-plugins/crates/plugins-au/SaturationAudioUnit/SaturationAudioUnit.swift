// SaturationAudioUnit.swift
// SOTF Saturation Audio Unit

import AVFoundation

public class SaturationAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Saturation" }
    override public class var pluginSubtype: String { "SOSt" }
    override public class var pluginName: String { "SOTF: Saturation" }
}
