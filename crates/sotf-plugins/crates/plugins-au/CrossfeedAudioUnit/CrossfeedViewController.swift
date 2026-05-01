import AppKit
import CoreAudioKit
import AudioToolbox

public class CrossfeedViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Crossfeed" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try CrossfeedAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
