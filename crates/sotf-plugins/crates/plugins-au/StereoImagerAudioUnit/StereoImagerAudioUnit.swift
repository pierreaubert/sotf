// StereoImagerAudioUnit.swift
// SOTF Stereo Imager Audio Unit

import AVFoundation

public class StereoImagerAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "StereoImager" }
    override public class var pluginSubtype: String { "SOSi" }
    override public class var pluginName: String { "SOTF: Stereo Imager" }
}
