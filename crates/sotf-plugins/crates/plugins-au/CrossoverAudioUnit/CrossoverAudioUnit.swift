// CrossoverAudioUnit.swift
// SOTF Crossover Audio Unit

import AVFoundation

public class CrossoverAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Crossover" }
    override public class var pluginSubtype: String { "SOCx" }
    override public class var pluginName: String { "SOTF: Crossover" }
}
