import AppKit
import CoreAudioKit
import AudioToolbox

public class BandSplitViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "BandSplit" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try BandSplitAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
