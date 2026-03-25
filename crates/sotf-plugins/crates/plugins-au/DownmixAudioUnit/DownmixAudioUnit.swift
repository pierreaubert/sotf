// DownmixAudioUnit.swift
// SOTF Downmix Audio Unit

import AVFoundation

public class DownmixAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Downmix" }
    override public class var pluginSubtype: String { "SODm" }
    override public class var pluginName: String { "SOTF: Downmix" }
}
