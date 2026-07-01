import SwiftUI
import UniformTypeIdentifiers

struct ImportFilePicker: UIViewControllerRepresentable {
    @EnvironmentObject var homeState: HomeState

    func makeUIViewController(context: Context) -> UIDocumentPickerViewController {
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: [UTType.item], asCopy: true)
        picker.allowsMultipleSelection = true
        picker.delegate = context.coordinator
        return picker
    }

    func updateUIViewController(_: UIDocumentPickerViewController, context _: Context) {}

    func makeCoordinator() -> Coordinator {
        Coordinator(homeState: homeState)
    }

    class Coordinator: NSObject, UIDocumentPickerDelegate {
        let homeState: HomeState

        init(homeState: HomeState) {
            self.homeState = homeState
        }

        func documentPicker(_: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
            for url in urls {
                _ = url.startAccessingSecurityScopedResource()
            }

            homeState.selectSheetInfo = .externalImport(urls: urls)
        }
    }
}
