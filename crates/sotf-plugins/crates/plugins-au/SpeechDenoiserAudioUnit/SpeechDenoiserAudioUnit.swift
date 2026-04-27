// SpeechDenoiserAudioUnit.swift
// SOTF Speech Denoiser Audio Unit

import AVFoundation

public class SpeechDenoiserAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "SpeechDenoiser" }
    override public class var pluginSubtype: String { "SOSd" }
    override public class var pluginName: String { "SOTF: Speech Denoiser" }
}
