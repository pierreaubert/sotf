// SpectrumAnalyzerAudioUnit.swift
// SOTF Spectrum Analyzer Audio Unit

import AVFoundation

public class SpectrumAnalyzerAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "SpectrumAnalyzer" }
    override public class var pluginSubtype: String { "SOSa" }
    override public class var pluginName: String { "SOTF: Spectrum Analyzer" }
}
