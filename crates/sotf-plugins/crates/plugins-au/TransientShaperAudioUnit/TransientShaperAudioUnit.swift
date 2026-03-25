// TransientShaperAudioUnit.swift
// SOTF Transient Shaper Audio Unit

import AVFoundation

public class TransientShaperAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "TransientShaper" }
    override public class var pluginSubtype: String { "SOTs" }
    override public class var pluginName: String { "SOTF: Transient Shaper" }
}
