import SwiftUI
import SwiftWorkspace

struct DeleteConfirmationButtons: View {
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    
    var files: [File]
    var deletedFilesMsg: String {
        get {
            return files.count == 1 ? "\"\(files[0].name)\"" : "\(files.count) files"
        }
    }
    
    var body: some View {
        Group {
            Button("Delete \(deletedFilesMsg)", role: .destructive) {
                filesModel.deleteFiles(files: files, workspaceInput: workspaceInput)
            }
            
            Button("Cancel", role: .cancel) {}
        }
    }
}
