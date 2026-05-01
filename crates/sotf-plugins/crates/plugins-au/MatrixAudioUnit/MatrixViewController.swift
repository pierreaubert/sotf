import AppKit
import CoreAudioKit
import AudioToolbox

public class MatrixViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Matrix" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try MatrixAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
