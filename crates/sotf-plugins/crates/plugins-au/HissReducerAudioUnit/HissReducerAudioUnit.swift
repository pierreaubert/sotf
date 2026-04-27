// HissReducerAudioUnit.swift
// SOTF Hiss Reducer Audio Unit

import AVFoundation

public class HissReducerAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "HissReducer" }
    override public class var pluginSubtype: String { "SOHr" }
    override public class var pluginName: String { "SOTF: Hiss Reducer" }
}
