import AppKit
import CoreAudioKit
import AudioToolbox

public class BinauralViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Binaural" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try BinauralAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
