import AppKit
import CoreAudioKit
import AudioToolbox

public class EQViewController: AUViewController, AUAudioUnitFactory {
    nonisolated(unsafe) private var audioUnit: EQAudioUnit?
    private var gpuiView: GPUIAUView?

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        NSLog("SOTF EQViewController: createAudioUnit called")
        let unit = try EQAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }

    public override func viewDidLoad() {
        super.viewDidLoad()
        NSLog("SOTF EQViewController: viewDidLoad, view.bounds = \(view.bounds)")

        // Blue background to distinguish VC's view from GPUIAUView's red
        view.wantsLayer = true
        view.layer?.backgroundColor = NSColor.blue.cgColor

        let gpui = GPUIAUView(pluginType: "EQ")
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
        NSLog("SOTF EQViewController: viewDidLayout, view.bounds = \(view.bounds), gpuiView.bounds = \(gpuiView?.bounds ?? .zero)")
    }
}
