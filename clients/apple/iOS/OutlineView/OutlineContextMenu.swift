import SwiftUI
import SwiftLockbookCore

struct OutlineContextMenu: View {
    
    let meta: File
    @State var branchState: BranchState?

    @EnvironmentObject var sheets: SheetState

    var body: some View {
        Text(meta.name)
        VStack {
            if meta.fileType == .Folder {
                Button(action: {
                    sheets.creatingInfo = CreatingInfo(parent: meta, child_type: .Document)
                }) {
                    Label("Create a document", systemImage: "doc")
                }
                Button(action: {
                    branchState?.open = true
                    sheets.creatingInfo = CreatingInfo(parent: meta, child_type: .Folder)
                }) {
                    Label("Create a folder", systemImage: "folder")
                }
            }
            if !meta.isRoot {
                Button(action: { sheets.renamingInfo = meta }) {
                    Label("Rename", systemImage: "questionmark.folder")
                }
                Button(action: { sheets.movingInfo = meta }) {
                    Label("Move", systemImage: "arrow.up.and.down.and.arrow.left.and.right")
                }
                Button(action: { DI.files.deleteFile(id: meta.id) }) {
                    Label("Delete", systemImage: "trash.fill")
                }
            }
        }
    }
}
