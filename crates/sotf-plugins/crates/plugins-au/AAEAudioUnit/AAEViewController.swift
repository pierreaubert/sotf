import AppKit
import CoreAudioKit
import AudioToolbox

public class AAEViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "AAE" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try AAEAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
