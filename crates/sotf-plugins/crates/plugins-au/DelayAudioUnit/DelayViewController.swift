import AppKit
import CoreAudioKit
import AudioToolbox

public class DelayViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Delay" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try DelayAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
