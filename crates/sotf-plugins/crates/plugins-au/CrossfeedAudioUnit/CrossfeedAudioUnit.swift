// CrossfeedAudioUnit.swift
// SOTF Crossfeed Audio Unit

import AVFoundation

public class CrossfeedAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Crossfeed" }
    override public class var pluginSubtype: String { "SOCf" }
    override public class var pluginName: String { "SOTF: Crossfeed" }
}
