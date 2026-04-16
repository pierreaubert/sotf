// AECAudioUnit.swift
// SOTF Echo Cancellation Audio Unit

import AVFoundation

public class AECAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "AEC" }
    override public class var pluginSubtype: String { "SOEc" }
    override public class var pluginName: String { "SOTF: Echo Cancellation" }
}
