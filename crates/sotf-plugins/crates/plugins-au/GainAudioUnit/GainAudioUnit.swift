// GainAudioUnit.swift
// SOTF Gain Audio Unit

import AVFoundation

public class GainAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Gain" }
    override public class var pluginSubtype: String { "SOGN" }
    override public class var pluginName: String { "SOTF: Gain" }
}
