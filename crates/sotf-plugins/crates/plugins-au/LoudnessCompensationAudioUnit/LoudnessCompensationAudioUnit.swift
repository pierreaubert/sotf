// LoudnessCompensationAudioUnit.swift
// SOTF Loudness Compensation Audio Unit

import AVFoundation

public class LoudnessCompensationAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "LoudnessCompensation" }
    override public class var pluginSubtype: String { "SOLc" }
    override public class var pluginName: String { "SOTF: Loudness Compensation" }
}
