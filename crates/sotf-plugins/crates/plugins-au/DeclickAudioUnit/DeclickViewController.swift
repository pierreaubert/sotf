import AppKit
import CoreAudioKit
import AudioToolbox

public class DeclickViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Declick" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try DeclickAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
