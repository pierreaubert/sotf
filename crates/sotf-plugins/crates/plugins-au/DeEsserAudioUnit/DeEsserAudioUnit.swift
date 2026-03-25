// DeEsserAudioUnit.swift
// SOTF De-Esser Audio Unit

import AVFoundation

public class DeEsserAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "DeEsser" }
    override public class var pluginSubtype: String { "SODs" }
    override public class var pluginName: String { "SOTF: De-Esser" }
}
