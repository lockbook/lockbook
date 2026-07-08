#if os(iOS)
    import SwiftUI
    import SwiftWorkspace
    import UIKit

    struct CameraView: UIViewControllerRepresentable {
        @Environment(WorkspaceInputState.self) private var workspaceInput
        @Environment(WorkspaceOutputState.self) private var workspaceOutput

        func makeUIViewController(context: Context) -> UIImagePickerController {
            let picker = UIImagePickerController()
            picker.sourceType = .camera
            picker.cameraCaptureMode = .photo
            picker.delegate = context.coordinator
            picker.showsCameraControls = true
            return picker
        }

        func updateUIViewController(_: UIImagePickerController, context _: Context) {}

        func makeCoordinator() -> Coordinator {
            Coordinator(self)
        }

        class Coordinator: NSObject, UIImagePickerControllerDelegate, UINavigationControllerDelegate {
            let parent: CameraView

            init(_ parent: CameraView) {
                self.parent = parent
            }

            func imagePickerController(
                _: UIImagePickerController,
                didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]
            ) {
                defer {
                    parent.workspaceOutput.openCamera = false
                }

                guard let image = info[.originalImage] as? UIImage,
                      let data = image.normalizedImage().jpegData(compressionQuality: 1.0)
                else {
                    AppState.shared.error = .custom(title: "Could not save image", msg: "")
                    return
                }

                parent.workspaceInput.pasteImage(data: data, isPaste: false)
            }

            func imagePickerControllerDidCancel(_: UIImagePickerController) {
                parent.workspaceOutput.openCamera = false
            }
        }
    }
#endif
