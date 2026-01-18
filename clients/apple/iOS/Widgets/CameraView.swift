import SwiftUI
import SwiftWorkspace
import UIKit
import UniformTypeIdentifiers

struct CameraView: UIViewControllerRepresentable {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var workspaceInput: WorkspaceInputState

    func makeUIViewController(context: Context) -> UIImagePickerController {
        let picker = UIImagePickerController()
        picker.sourceType = .camera
        picker.cameraCaptureMode = .photo
        picker.delegate = context.coordinator
        picker.showsCameraControls = true
        return picker
    }

    func updateUIViewController(
        _ uiViewController: UIImagePickerController,
        context: Context
    ) {}

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    class Coordinator: NSObject, UIImagePickerControllerDelegate,
        UINavigationControllerDelegate
    {
        let parent: CameraView
        init(_ parent: CameraView) { self.parent = parent }

        func imagePickerController(
            _ picker: UIImagePickerController,
            didFinishPickingMediaWithInfo info: [UIImagePickerController
                .InfoKey: Any]
        ) {
            guard let image = info[.originalImage] as? UIImage,
                let data = image.normalizedImage()?.jpegData(
                    compressionQuality: 1.0
                )
            else {
                AppState.shared.error = .custom(
                    title: "Could not save image",
                    msg: ""
                )

                return
            }

            parent.workspaceInput.pasteImage(data: data, isPaste: false)
            parent.homeState.sheetInfo = nil
        }

        func imagePickerControllerDidCancel(_ picker: UIImagePickerController) {
            parent.homeState.sheetInfo = nil
        }
    }
}

extension UIImage {
    func normalizedImage() -> UIImage? {
        if imageOrientation == .up { return self }

        let format = UIGraphicsImageRendererFormat.default()
        format.scale = self.scale
        format.opaque = false

        let renderer = UIGraphicsImageRenderer(size: self.size, format: format)

        return renderer.image { context in
            self.draw(in: CGRect(origin: .zero, size: self.size))
        }
    }
}
