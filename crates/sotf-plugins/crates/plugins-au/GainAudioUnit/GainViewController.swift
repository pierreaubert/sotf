import AppKit
import CoreAudioKit
import AudioToolbox

public class GainViewController: AUViewController, AUAudioUnitFactory {
    nonisolated(unsafe) private var audioUnit: GainAudioUnit?

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try GainAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }

    public override func viewDidLoad() {
        super.viewDidLoad()
        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor(calibratedRed: 0.12, green: 0.12, blue: 0.14, alpha: 1.0).cgColor

        let label = NSTextField(labelWithString: "SOTF: Gain\nUse host controls for parameters")
        label.font = NSFont.systemFont(ofSize: 14, weight: .medium)
        label.textColor = NSColor(calibratedWhite: 0.7, alpha: 1.0)
        label.alignment = .center
        label.maximumNumberOfLines = 0
        label.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(label)

        NSLayoutConstraint.activate([
            label.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            label.centerYAnchor.constraint(equalTo: view.centerYAnchor),
        ])
    }
}
