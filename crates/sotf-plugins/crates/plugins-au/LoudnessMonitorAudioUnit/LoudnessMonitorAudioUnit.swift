// LoudnessMonitorAudioUnit.swift
// SOTF Loudness Monitor Audio Unit

import AVFoundation

public class LoudnessMonitorAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "LoudnessMonitor" }
    override public class var pluginSubtype: String { "SOLu" }
    override public class var pluginName: String { "SOTF: Loudness Monitor" }
}
