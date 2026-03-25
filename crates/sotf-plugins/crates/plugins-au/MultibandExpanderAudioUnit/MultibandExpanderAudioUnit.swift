// MultibandExpanderAudioUnit.swift
// SOTF Multiband Expander Audio Unit

import AVFoundation

public class MultibandExpanderAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "MultibandExpander" }
    override public class var pluginSubtype: String { "SOMe" }
    override public class var pluginName: String { "SOTF: Multiband Expander" }
}
