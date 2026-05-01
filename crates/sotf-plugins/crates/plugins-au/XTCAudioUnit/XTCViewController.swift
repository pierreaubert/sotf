import AppKit
import CoreAudioKit
import AudioToolbox

public class XTCViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "XTC" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try XTCAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
