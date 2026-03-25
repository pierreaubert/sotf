// XTCAudioUnit.swift
// SOTF XTC Audio Unit

import AVFoundation

public class XTCAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "XTC" }
    override public class var pluginSubtype: String { "SOXt" }
    override public class var pluginName: String { "SOTF: XTC" }
}
