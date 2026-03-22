// CompressorAudioUnit.swift
// SOTF Compressor Audio Unit

import AVFoundation

public class CompressorAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Compressor" }
    override public class var pluginSubtype: String { "SOCP" }
    override public class var pluginName: String { "SOTF: Compressor" }
}
