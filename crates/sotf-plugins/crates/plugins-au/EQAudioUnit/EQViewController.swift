import AppKit
import CoreAudioKit
import AudioToolbox

public class EQViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "EQ" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try EQAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
