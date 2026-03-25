import AppKit
import CoreAudioKit
import AudioToolbox

public class EQViewController: AUViewController, AUAudioUnitFactory {
    nonisolated(unsafe) private var audioUnit: EQAudioUnit?
    nonisolated(unsafe) private var gpuiView: GPUIAUView?

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        NSLog("SOTF EQViewController: createAudioUnit called")
        let unit = try EQAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }

    public override func viewDidLoad() {
        super.viewDidLoad()
        NSLog("SOTF EQViewController: viewDidLoad, view.bounds = \(view.bounds)")

        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor.blue.cgColor

        let gpui = GPUIAUView(pluginType: "EQ", audioUnit: audioUnit)
        gpui.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(gpui)
        NSLayoutConstraint.activate([
            gpui.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            gpui.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            gpui.topAnchor.constraint(equalTo: view.topAnchor),
            gpui.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        gpuiView = gpui
    }

    public override func viewDidLayout() {
        super.viewDidLayout()
        // Late-bind AU if createAudioUnit was called after viewDidLoad
        if let au = audioUnit, let gpui = gpuiView {
            gpui.connectAudioUnit(au)
        }
    }
}
