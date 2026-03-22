// LimiterAudioUnit.swift
// SOTF Limiter Audio Unit

import AVFoundation

public class LimiterAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Limiter" }
    override public class var pluginSubtype: String { "SOLM" }
    override public class var pluginName: String { "SOTF: Limiter" }
}
