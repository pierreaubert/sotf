// DitherAudioUnit.swift
// SOTF Dither Audio Unit

import AVFoundation

public class DitherAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Dither" }
    override public class var pluginSubtype: String { "SODt" }
    override public class var pluginName: String { "SOTF: Dither" }
}
