// DenoiserAudioUnit.swift
// SOTF Denoiser Audio Unit

import AVFoundation

public class DenoiserAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Denoiser" }
    override public class var pluginSubtype: String { "SODn" }
    override public class var pluginName: String { "SOTF: Denoiser" }
}
