// BeamformerAudioUnit.swift
// SOTF Beamformer Audio Unit

import AVFoundation

public class BeamformerAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Beamformer" }
    override public class var pluginSubtype: String { "SOBF" }
    override public class var pluginName: String { "SOTF: Beamformer" }
}
