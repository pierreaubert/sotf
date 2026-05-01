import AppKit
import CoreAudioKit
import AudioToolbox

public class StereoImagerViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "StereoImager" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try StereoImagerAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
