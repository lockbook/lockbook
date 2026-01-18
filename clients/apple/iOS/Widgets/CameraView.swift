import SwiftUI
import SwiftWorkspace
import UIKit

struct CameraView: UIViewControllerRepresentable {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var workspaceInput: WorkspaceInputState

    func makeUIViewController(context: Context) -> UIImagePickerController {
        let picker = UIImagePickerController()
        picker.sourceType = .camera
        picker.delegate = context.coordinator
        picker.allowsEditing = true
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
            guard let image = info[.editedImage] as? UIImage,
                let data = image.pngData()
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
