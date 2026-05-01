import AppKit
import CoreAudioKit
import AudioToolbox

public class SpectralCompressorViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "SpectralCompressor" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try SpectralCompressorAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
