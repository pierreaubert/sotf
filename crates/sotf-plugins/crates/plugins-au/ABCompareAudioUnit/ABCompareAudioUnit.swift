// ABCompareAudioUnit.swift
// SOTF AB Compare Audio Unit

import AVFoundation

public class ABCompareAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "ABCompare" }
    override public class var pluginSubtype: String { "SOAb" }
    override public class var pluginName: String { "SOTF: AB Compare" }
}
