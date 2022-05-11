import SwiftUI
import SwiftLockbookCore

struct OutlineContextMenu: View {
    
    let meta: DecryptedFileMetadata
    @State var outlineState: OutlineState
    @State var branchState: BranchState?
    @State var creating = false
    
    var body: some View {
        Text(meta.decryptedName)
        VStack {
            if meta.fileType == .Folder {
                Button(action: {
                    outlineState.creatingInfo = CreatingInfo(parent: meta, child_type: .Document)
                }) {
                    Label("Create a document", systemImage: "doc")
                }
                Button(action: {
                    branchState?.open = true
                    outlineState.creatingInfo = CreatingInfo(parent: meta, child_type: .Folder)
                }) {
                    Label("Create a folder", systemImage: "folder")
                }
            }
            if !meta.isRoot {
                Button(action: { outlineState.renamingInfo = meta }) {
                    Label("Rename", systemImage: "questionmark.folder")
                }
                Button(action: { outlineState.movingInfo = meta }) {
                    Label("Move", systemImage: "arrow.up.and.down.and.arrow.left.and.right")
                }
                Button(action: { DI.files.deleteFile(id: meta.id) }) {
                    Label("Delete", systemImage: "trash.fill")
                }
            }
        }
    }
}
