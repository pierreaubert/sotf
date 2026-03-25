// FletcherMunsonAudioUnit.swift
// SOTF Fletcher-Munson Audio Unit

import AVFoundation

public class FletcherMunsonAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "FletcherMunson" }
    override public class var pluginSubtype: String { "SOFm" }
    override public class var pluginName: String { "SOTF: Fletcher-Munson" }
}
