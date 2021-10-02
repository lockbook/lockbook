import SwiftUI
import SwiftLockbookCore

struct OutlineContextMenu: View {
    
    @ObservedObject var outlineState: OutlineState
    @ObservedObject var branchState: BranchState
    
    let meta: ClientFileMetadata
    
    var body: some View {
        VStack {
            Text(meta.name)
            if meta.fileType == .Folder {
                Button(action: { branchState.creating = .Document }) {
                    Label("Create a document", systemImage: "doc")
                }
                Button(action: { branchState.creating = .Folder }) {
                    Label("Create a folder", systemImage: "folder")
                }
            }
            if !meta.isRoot {
                Button(action: { outlineState.renaming = meta }) {
                    Label("Rename", systemImage: "pencil")
                }
                Button(action: { DI.files.deleteFile(id: meta.id) }) {
                    Label("Delete", systemImage: "trash.fill")
                }
            }
        }
    }
}
