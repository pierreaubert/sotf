import AppKit
import CoreAudioKit
import AudioToolbox

public class CrossoverViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Crossover" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try CrossoverAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
