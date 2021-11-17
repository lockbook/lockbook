import SwiftUI
import SwiftLockbookCore

struct OutlineContextMenu {
    static func getContextView(meta: DecryptedFileMetadata, outlineState: OutlineState, branchState: BranchState?) -> some View {
        VStack {
            Text(meta.decryptedName)
            if meta.fileType == .Folder {
                Button(action: {
                    branchState?.open = true
                    outlineState.creating = CreatingInfo(parent: meta, child_type: .Document)
                }) {
                    Label("Create a document", systemImage: "doc")
                }
                Button(action: {
                    branchState?.open = true
                    outlineState.creating = CreatingInfo(parent: meta, child_type: .Folder)
                }) {
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
