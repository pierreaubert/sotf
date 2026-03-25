// ConvolutionAudioUnit.swift
// SOTF Convolution Audio Unit

import AVFoundation

public class ConvolutionAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Convolution" }
    override public class var pluginSubtype: String { "SOCv" }
    override public class var pluginName: String { "SOTF: Convolution" }
}
