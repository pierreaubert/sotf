import AppKit
import CoreAudioKit
import AudioToolbox

public class CompressorViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Compressor" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try CompressorAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
