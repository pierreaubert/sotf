// ExpanderAudioUnit.swift
// SOTF Expander Audio Unit

import AVFoundation

public class ExpanderAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Expander" }
    override public class var pluginSubtype: String { "SOEx" }
    override public class var pluginName: String { "SOTF: Expander" }
}
