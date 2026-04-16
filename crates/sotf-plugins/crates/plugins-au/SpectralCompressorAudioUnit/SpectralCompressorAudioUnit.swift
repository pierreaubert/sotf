// SpectralCompressorAudioUnit.swift
// SOTF Spectral Compressor Audio Unit

import AVFoundation

public class SpectralCompressorAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "SpectralCompressor" }
    override public class var pluginSubtype: String { "SOSC" }
    override public class var pluginName: String { "SOTF: Spectral Compressor" }
}
