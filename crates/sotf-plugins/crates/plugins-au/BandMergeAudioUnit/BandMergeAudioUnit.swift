// BandMergeAudioUnit.swift
// SOTF Band Merge Audio Unit

import AVFoundation

public class BandMergeAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "BandMerge" }
    override public class var pluginSubtype: String { "SOBM" }
    override public class var pluginName: String { "SOTF: Band Merge" }
}
