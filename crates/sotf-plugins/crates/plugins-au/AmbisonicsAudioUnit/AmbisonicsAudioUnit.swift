// AmbisonicsAudioUnit.swift
// SOTF Ambisonics Decoder Audio Unit

import AVFoundation

public class AmbisonicsAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "AmbisonicsDecoder" }
    override public class var pluginSubtype: String { "SOAm" }
    override public class var pluginName: String { "SOTF: Ambisonics Decoder" }
}
