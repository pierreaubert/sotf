import AppKit
import CoreAudioKit
import AudioToolbox

public class SaturationViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Saturation" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try SaturationAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
