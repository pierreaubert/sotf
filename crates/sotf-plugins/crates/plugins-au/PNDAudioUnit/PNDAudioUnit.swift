// PNDAudioUnit.swift
// SOTF PND Audio Unit

import AVFoundation

public class PNDAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "PND" }
    override public class var pluginSubtype: String { "SOPn" }
    override public class var pluginName: String { "SOTF: PND" }
}
