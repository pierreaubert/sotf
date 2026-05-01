import AppKit
import CoreAudioKit
import AudioToolbox

public class ExpanderViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Expander" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try ExpanderAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
