// BinauralAudioUnit.swift
// SOTF Binaural Audio Unit

import AVFoundation

public class BinauralAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Binaural" }
    override public class var pluginSubtype: String { "SOBn" }
    override public class var pluginName: String { "SOTF: Binaural" }
}
