// EQAudioUnit.swift
// SOTF Parametric EQ Audio Unit — delegates to Rust plugin via GenericRustAudioUnit

import AVFoundation

public class EQAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "EQ" }
    override public class var pluginSubtype: String { "SOEQ" }
    override public class var pluginName: String { "SOTF: Parametric EQ" }
}
