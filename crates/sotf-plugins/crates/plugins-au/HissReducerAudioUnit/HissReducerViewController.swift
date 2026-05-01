import AppKit
import CoreAudioKit
import AudioToolbox

public class HissReducerViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "HissReducer" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try HissReducerAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
