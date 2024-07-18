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
                    Label("Create a document", systemImage: "doc.fill")
                }
                Button(action: {
                    withAnimation {
                        branchState?.open = true
                    }
                    DI.files.createDoc(maybeParent: meta.id, isDrawing: true)
                }) {
                    Label("Create a drawing", systemImage: "pencil.tip.crop.circle.badge.plus")
                }
                Button(action: {
                    withAnimation {
                        branchState?.open = true
                    }
                    DI.sheets.creatingFolderInfo = CreatingFolderInfo(parentPath: DI.files.getPathByIdOrParent(maybeId: meta.id) ?? "ERROR", maybeParent: meta.id)
                }) {
                    Label("Create a folder", systemImage: "folder.fill")
                }
            }
            
            if !meta.isRoot {
                Button(action: {
                    DI.sheets.renamingFileInfo = RenamingFileInfo(id: meta.id, name: meta.name, parentPath: DI.files.getPathByIdOrParent(maybeId: meta.parent) ?? "ERROR")
                }) {
                    Label("Rename", systemImage: "pencil.circle.fill")
                }

                Button(action: { sheets.movingInfo = meta }) {
                    Label("Move", systemImage: "arrow.up.and.down.and.arrow.left.and.right")
                }
                
                Button(action: { DI.sheets.sharingFileInfo = meta}) {
                    Label("Share", systemImage: "person.wave.2.fill")
                }

                Button(action: { exportFileAndShowShareSheet(meta: meta) }) {
                    Label("Share externally to...", systemImage: "square.and.arrow.up.fill")
                }
                
                if meta.fileType == .Document {
                    Button(action: {
                        DI.files.copyFileLink(id: meta.id)
                    }) {
                        Label("Copy file link", systemImage: "link")
                    }
                }
                
                Button(role: .destructive, action: {
                    DI.sheets.deleteConfirmationInfo = meta
                }) {
                    Label("Delete", systemImage: "trash.fill")
                }
            }
        }
    }
}
