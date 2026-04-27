// DeclickAudioUnit.swift
// SOTF Declick Audio Unit

import AVFoundation

public class DeclickAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Declick" }
    override public class var pluginSubtype: String { "SODc" }
    override public class var pluginName: String { "SOTF: Declick" }
}
