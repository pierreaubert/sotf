// AAEAudioUnit.swift
// SOTF AAE Reverb Audio Unit

import AVFoundation

public class AAEAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "AAE" }
    override public class var pluginSubtype: String { "SOAE" }
    override public class var pluginName: String { "SOTF: AAE Reverb" }
}
