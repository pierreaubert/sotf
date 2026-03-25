// DynamicEQAudioUnit.swift
// SOTF Dynamic EQ Audio Unit

import AVFoundation

public class DynamicEQAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "DynamicEQ" }
    override public class var pluginSubtype: String { "SODq" }
    override public class var pluginName: String { "SOTF: Dynamic EQ" }
}
