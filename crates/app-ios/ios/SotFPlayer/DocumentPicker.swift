import UIKit
import UniformTypeIdentifiers

/// Presents a document picker for importing audio files into the app sandbox.
///
/// Selected files are copied into Documents/Music/ and the Rust side is
/// notified via FFI so the library scanner can pick them up.
class DocumentPicker: NSObject, UIDocumentPickerDelegate {

    static let shared = DocumentPicker()

    /// Music directory inside the app sandbox
    static var musicDirectory: URL {
        let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
        let musicDir = docs.appendingPathComponent("Music", isDirectory: true)
        try? FileManager.default.createDirectory(at: musicDir, withIntermediateDirectories: true)
        return musicDir
    }

    /// Present the document picker from the key window's root view controller
    func presentPicker() {
        let supportedTypes: [UTType] = [
            .audio,
            .mp3,
            .mpeg4Audio,
            UTType("public.flac") ?? .audio,
            UTType("org.xiph.ogg-vorbis") ?? .audio,
            UTType("com.microsoft.waveform-audio") ?? .audio,
            .folder
        ]

        let picker = UIDocumentPickerViewController(forOpeningContentTypes: supportedTypes, asCopy: true)
        picker.delegate = self
        picker.allowsMultipleSelection = true

        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let rootVC = windowScene.windows.first?.rootViewController else {
            NSLog("[DocumentPicker] No root view controller to present from")
            return
        }

        rootVC.present(picker, animated: true)
    }

    // MARK: - UIDocumentPickerDelegate

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        let musicDir = DocumentPicker.musicDirectory
        var importedPaths: [String] = []

        for url in urls {
            let destination = musicDir.appendingPathComponent(url.lastPathComponent)

            // Skip if file already exists at destination
            if FileManager.default.fileExists(atPath: destination.path) {
                NSLog("[DocumentPicker] File already exists, skipping: \(url.lastPathComponent)")
                importedPaths.append(destination.path)
                continue
            }

            do {
                // asCopy: true means the file is already copied to a temp location.
                // Move it to our Music directory.
                try FileManager.default.moveItem(at: url, to: destination)
                importedPaths.append(destination.path)
                NSLog("[DocumentPicker] Imported: \(destination.lastPathComponent)")
            } catch {
                NSLog("[DocumentPicker] Failed to import \(url.lastPathComponent): \(error)")
            }
        }

        if !importedPaths.isEmpty {
            // Notify Rust that files were imported
            let json = try? JSONSerialization.data(
                withJSONObject: importedPaths,
                options: []
            )
            if let json = json, let jsonStr = String(data: json, encoding: .utf8) {
                jsonStr.withCString { cStr in
                    sotf_ios_files_imported(cStr)
                }
            }
        }
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        NSLog("[DocumentPicker] Cancelled")
    }
}
