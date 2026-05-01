import AppKit
import CoreAudioKit
import AudioToolbox

public class ABCompareViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "ABCompare" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try ABCompareAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
