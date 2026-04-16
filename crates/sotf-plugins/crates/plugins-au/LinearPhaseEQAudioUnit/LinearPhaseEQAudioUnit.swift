// LinearPhaseEQAudioUnit.swift
// SOTF Linear-Phase EQ Audio Unit

import AVFoundation

public class LinearPhaseEQAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "LinearPhaseEQ" }
    override public class var pluginSubtype: String { "SOLP" }
    override public class var pluginName: String { "SOTF: Linear-Phase EQ" }
}
