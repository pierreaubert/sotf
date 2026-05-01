import AppKit
import CoreAudioKit
import AudioToolbox

public class MultibandExpanderViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "MultibandExpander" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try MultibandExpanderAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
