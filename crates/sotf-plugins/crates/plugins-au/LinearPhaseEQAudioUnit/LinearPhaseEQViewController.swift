import AppKit
import CoreAudioKit
import AudioToolbox

public class LinearPhaseEQViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "LinearPhaseEQ" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try LinearPhaseEQAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
