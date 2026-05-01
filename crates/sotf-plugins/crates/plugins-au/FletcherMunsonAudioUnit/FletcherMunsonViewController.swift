import AppKit
import CoreAudioKit
import AudioToolbox

public class FletcherMunsonViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "FletcherMunson" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try FletcherMunsonAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
