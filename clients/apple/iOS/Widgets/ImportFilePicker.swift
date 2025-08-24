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

    func updateUIViewController(_ uiViewController: UIDocumentPickerViewController, context: Context) {}

    func makeCoordinator() -> Coordinator {
        Coordinator(homeState: homeState)
    }

    class Coordinator: NSObject, UIDocumentPickerDelegate {
        let homeState: HomeState
        
        init(homeState: HomeState) {
            self.homeState = homeState
        }
        
        func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
            for url in urls {
                let _ = url.startAccessingSecurityScopedResource()
            }
            
            homeState.selectSheetInfo = .externalImport(urls: urls)
        }
    }
}
