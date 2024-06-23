import SwiftUI
import SwiftLockbookCore
import Introspect

struct FileCell: View {
    let meta: File

    let enterFolderAnim: (File) -> Void

    var body: some View {
        cell
            .contextMenu(menuItems: {
                // TODO: disast: https://stackoverflow.com/questions/70159437/context-menu-not-updating-in-swiftui

                Button(action: {
                    DI.sheets.renamingFileInfo = RenamingFileInfo(id: meta.id, name: meta.name, parentPath: DI.files.getPathByIdOrParent(maybeId: meta.parent) ?? "ERROR")
                }) {
                    Label("Rename", systemImage: "pencil.circle.fill")
                }
                                
                Button(action: {
                    DI.sheets.movingInfo = meta
                }, label: {
                    Label("Move", systemImage: "arrow.up.and.down.and.arrow.left.and.right")
                })
                
                Button(action: {
                    DI.files.deleteFile(id: meta.id)
                }) {
                    Label("Delete", systemImage: "trash.fill")
                }
                
                Button(action: {
                    DI.sheets.sharingFileInfo = meta
                }, label: {
                    Label("Share", systemImage: "person.wave.2.fill")
                })
                
                Button(action: {
                    exportFileAndShowShareSheet(meta: meta)
                }, label: {
                    Label("Share externally to...", systemImage: "square.and.arrow.up.fill")
                })
                
                if meta.fileType == .Document {
                    Button(action: {
                        DI.files.copyFileLink(id: meta.id)
                    }) {
                        Label("Copy file link", systemImage: "link")
                    }
                }
            })
    }

    @ViewBuilder
    var cell: some View {
        Button(action: {
            if meta.fileType == .Folder {
                enterFolderAnim(meta)
            } else {
                withAnimation {
                    DI.workspace.requestOpenDoc(meta.id)
                }
            }
        }) {
            RealFileCell(meta: meta)
        }
    }
}

struct RealFileCell: View {
    let meta: File

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(meta.name)
                    .font(.title3)
            HStack {
                Image(systemName: meta.fileType == .Folder ? "folder.fill" : documentExtensionToImage(name: meta.name))
                        .foregroundColor(meta.fileType == .Folder ? .blue : .secondary)
                Text(intEpochToString(epoch: max(meta.lastModified, meta.lastModified)))
                        .foregroundColor(.secondary)
                
                Spacer()
            }
                    .font(.footnote)
        }
            .padding(.vertical, 5)
            .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
    }
}

public func documentExtensionToImage(name: String) -> String {
    if name.hasSuffix(".md") {
        return "doc.plaintext"
    } else if name.hasSuffix(".draw") {
        return "doc.richtext"
    } else {
        return "doc"
    }
}
