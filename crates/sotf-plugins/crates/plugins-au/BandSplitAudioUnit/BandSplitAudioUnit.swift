// BandSplitAudioUnit.swift
// SOTF Band Split Audio Unit

import AVFoundation

public class BandSplitAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "BandSplit" }
    override public class var pluginSubtype: String { "SOBS" }
    override public class var pluginName: String { "SOTF: Band Split" }
}
