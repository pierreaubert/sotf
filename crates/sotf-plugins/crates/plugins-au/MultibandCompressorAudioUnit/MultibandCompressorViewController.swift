import AppKit
import CoreAudioKit
import AudioToolbox

public class MultibandCompressorViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "MultibandCompressor" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try MultibandCompressorAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
