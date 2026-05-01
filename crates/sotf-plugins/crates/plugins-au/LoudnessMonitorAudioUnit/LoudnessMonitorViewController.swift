import AppKit
import CoreAudioKit
import AudioToolbox

public class LoudnessMonitorViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "LoudnessMonitor" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try LoudnessMonitorAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
