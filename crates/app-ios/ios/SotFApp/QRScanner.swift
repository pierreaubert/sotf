import AVFoundation
import UIKit

/// Native QR scanner for importing SOTF API connection payloads.
final class QRScanner: NSObject, AVCaptureMetadataOutputObjectsDelegate {

    static let shared = QRScanner()

    private var session: AVCaptureSession?
    private var scannerViewController: UIViewController?
    private var didScan = false

    func presentScanner() {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized:
            presentAuthorizedScanner()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { granted in
                DispatchQueue.main.async {
                    if granted {
                        self.presentAuthorizedScanner()
                    } else {
                        self.showAlert(
                            title: "Camera Access Required",
                            message: "Allow camera access to scan a SOTF server QR code."
                        )
                    }
                }
            }
        case .denied, .restricted:
            showAlert(
                title: "Camera Access Required",
                message: "Allow camera access in Settings to scan a SOTF server QR code."
            )
        @unknown default:
            showAlert(
                title: "Camera Unavailable",
                message: "This device cannot scan QR codes right now."
            )
        }
    }

    private func presentAuthorizedScanner() {
        guard scannerViewController == nil else {
            return
        }
        guard let rootVC = QRScanner.rootViewController() else {
            NSLog("[QRScanner] No root view controller to present from")
            return
        }
        guard let device = AVCaptureDevice.default(for: .video) else {
            showAlert(
                title: "Camera Unavailable",
                message: "This device does not expose a camera to scan QR codes."
            )
            return
        }

        let captureSession = AVCaptureSession()
        do {
            let input = try AVCaptureDeviceInput(device: device)
            guard captureSession.canAddInput(input) else {
                showAlert(title: "Camera Unavailable", message: "The camera input cannot be used.")
                return
            }
            captureSession.addInput(input)
        } catch {
            NSLog("[QRScanner] Failed to create camera input: \(error)")
            showAlert(title: "Camera Unavailable", message: "The camera input cannot be used.")
            return
        }

        let metadataOutput = AVCaptureMetadataOutput()
        guard captureSession.canAddOutput(metadataOutput) else {
            showAlert(title: "QR Scanner Unavailable", message: "QR metadata scanning is not available.")
            return
        }
        captureSession.addOutput(metadataOutput)
        metadataOutput.setMetadataObjectsDelegate(self, queue: DispatchQueue.main)
        if metadataOutput.availableMetadataObjectTypes.contains(.qr) {
            metadataOutput.metadataObjectTypes = [.qr]
        } else {
            showAlert(title: "QR Scanner Unavailable", message: "This camera cannot scan QR codes.")
            return
        }

        let controller = UIViewController()
        controller.view.backgroundColor = .black
        controller.modalPresentationStyle = .fullScreen

        let previewLayer = AVCaptureVideoPreviewLayer(session: captureSession)
        previewLayer.videoGravity = .resizeAspectFill
        previewLayer.frame = rootVC.view.bounds
        controller.view.layer.addSublayer(previewLayer)

        let cancelButton = UIButton(type: .system)
        cancelButton.setTitle("Cancel", for: .normal)
        cancelButton.setTitleColor(.white, for: .normal)
        cancelButton.titleLabel?.font = .systemFont(ofSize: 18, weight: .semibold)
        cancelButton.backgroundColor = UIColor.black.withAlphaComponent(0.55)
        cancelButton.layer.cornerRadius = 10
        cancelButton.translatesAutoresizingMaskIntoConstraints = false
        cancelButton.addTarget(self, action: #selector(cancelScanner), for: .touchUpInside)
        controller.view.addSubview(cancelButton)
        NSLayoutConstraint.activate([
            cancelButton.topAnchor.constraint(equalTo: controller.view.safeAreaLayoutGuide.topAnchor, constant: 16),
            cancelButton.trailingAnchor.constraint(equalTo: controller.view.safeAreaLayoutGuide.trailingAnchor, constant: -16),
            cancelButton.widthAnchor.constraint(greaterThanOrEqualToConstant: 96),
            cancelButton.heightAnchor.constraint(equalToConstant: 44),
        ])

        didScan = false
        session = captureSession
        scannerViewController = controller
        rootVC.present(controller, animated: true) {
            DispatchQueue.global(qos: .userInitiated).async {
                captureSession.startRunning()
            }
        }
    }

    @objc private func cancelScanner() {
        dismissScanner()
    }

    func metadataOutput(
        _ output: AVCaptureMetadataOutput,
        didOutput metadataObjects: [AVMetadataObject],
        from connection: AVCaptureConnection
    ) {
        guard !didScan,
              let readable = metadataObjects.compactMap({ $0 as? AVMetadataMachineReadableCodeObject }).first,
              readable.type == .qr,
              let value = readable.stringValue else {
            return
        }

        didScan = true
        dismissScanner {
            value.withCString { cString in
                sotf_ios_qr_scanned(cString)
            }
        }
    }

    private func dismissScanner(completion: (() -> Void)? = nil) {
        let activeSession = session
        session = nil
        DispatchQueue.global(qos: .userInitiated).async {
            activeSession?.stopRunning()
        }

        guard let controller = scannerViewController else {
            completion?()
            return
        }
        scannerViewController = nil
        controller.dismiss(animated: true) {
            completion?()
        }
    }

    private func showAlert(title: String, message: String) {
        guard let rootVC = QRScanner.rootViewController() else {
            NSLog("[QRScanner] \(title): \(message)")
            return
        }
        let alert = UIAlertController(title: title, message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "OK", style: .default))
        rootVC.present(alert, animated: true)
    }

    private static func rootViewController() -> UIViewController? {
        UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .flatMap { $0.windows }
            .first { $0.isKeyWindow }?
            .rootViewController
            ?? UIApplication.shared.connectedScenes
                .compactMap { $0 as? UIWindowScene }
                .flatMap { $0.windows }
                .first?
                .rootViewController
    }
}

@_cdecl("sotf_ios_show_qr_scanner")
func sotfIosShowQrScanner() {
    DispatchQueue.main.async {
        QRScanner.shared.presentScanner()
    }
}
