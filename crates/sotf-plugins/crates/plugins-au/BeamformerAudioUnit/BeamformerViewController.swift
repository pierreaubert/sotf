import AppKit
import CoreAudioKit
import AudioToolbox

public class BeamformerViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Beamformer" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try BeamformerAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
