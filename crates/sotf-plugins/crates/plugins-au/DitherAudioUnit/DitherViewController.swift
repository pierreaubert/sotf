import AppKit
import CoreAudioKit
import AudioToolbox

public class DitherViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Dither" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try DitherAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
