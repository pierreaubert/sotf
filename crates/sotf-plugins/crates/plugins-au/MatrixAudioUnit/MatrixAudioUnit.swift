// MatrixAudioUnit.swift
// SOTF Matrix Audio Unit

import AVFoundation

public class MatrixAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "Matrix" }
    override public class var pluginSubtype: String { "SOMx" }
    override public class var pluginName: String { "SOTF: Matrix" }
}
