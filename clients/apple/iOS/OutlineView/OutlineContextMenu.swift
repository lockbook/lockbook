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
                    withAnimation {
                        branchState?.open = true
                    }
                    DI.files.createDoc(maybeParent: meta.id, isDrawing: false)
                }) {
                    Label("Create a document", systemImage: "doc")
                }
                Button(action: {
                    withAnimation {
                        branchState?.open = true
                    }
                    DI.files.createDoc(maybeParent: meta.id, isDrawing: true)
                }) {
                    Label("Create a drawing", systemImage: "doc")
                }
                Button(action: {
                    withAnimation {
                        branchState?.open = true
                    }
                    DI.sheets.creatingFolderInfo = CreatingFolderInfo(parentPath: DI.files.getPathByIdOrParent(maybeId: meta.id) ?? "ERROR", maybeParent: meta.id)
                }) {
                    Label("Create a folder", systemImage: "folder")
                }
            }
            
            if !meta.isRoot {
                Button(action: { sheets.movingInfo = meta }) {
                    Label("Move", systemImage: "arrow.up.and.down.and.arrow.left.and.right")
                }
                Button(action: { DI.files.deleteFile(id: meta.id) }) {
                    Label("Delete", systemImage: "trash.fill")
                }
                
                Button(action: { exportFileAndShowShareSheet(meta: meta) }) {
                    Label("Export", systemImage: "square.and.arrow.up")
                }
            }
        }
    }
}
