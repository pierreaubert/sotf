// MultibandCompressorAudioUnit.swift
// SOTF Multiband Compressor Audio Unit

import AVFoundation

public class MultibandCompressorAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "MultibandCompressor" }
    override public class var pluginSubtype: String { "SOMc" }
    override public class var pluginName: String { "SOTF: Multiband Compressor" }
}
