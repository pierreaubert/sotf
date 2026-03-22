// GateAudioUnit.swift
// SOTF Gate Audio Unit

import AVFoundation

public class GateAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Gate" }
    override public class var pluginSubtype: String { "SOGT" }
    override public class var pluginName: String { "SOTF: Gate" }
}
