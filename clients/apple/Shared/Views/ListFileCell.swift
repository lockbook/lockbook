import SwiftUI
import SwiftWorkspace
import SwiftLockbookCore
import Introspect

struct FileCell: View {
    let meta: File

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
                    DI.sheets.movingInfo = .Move([meta.id])
                }, label: {
                    Label("Move", systemImage: "arrow.up.and.down.and.arrow.left.and.right")
                })
                
                Divider()
                
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
                
                Divider()
                
                Button(role: .destructive, action: {
                    DI.sheets.deleteConfirmationInfo = meta
                }) {
                    Label("Delete", systemImage: "trash.fill")
                }
            })
    }

    @ViewBuilder
    var cell: some View {
        Button(action: {
            if meta.fileType == .Document {
                DI.workspace.requestOpenDoc(meta.id)
            }
            
            DI.files.intoChildDirectory(meta)
        }) {
            RealFileCell(meta: meta)
        }
    }
}

struct RealFileCell: View {
    let meta: File

    var body: some View {
        HStack(spacing: 20) {
            Image(systemName: FileService.metaToSystemImage(meta: meta))
                .foregroundColor(meta.fileType == .Folder ? .blue : .secondary)
                .font(.title3)
                .frame(width: 20)
            
            if meta.fileType == .Document {
                VStack(alignment: .leading) {
                    Text(meta.name)
                        .font(.body)
                        .lineLimit(1)
                    
                    Text(DI.core.timeAgo(timeStamp: Int64(meta.lastModified)))
                            .foregroundColor(.secondary)
                            .font(.caption)
                }
            } else {
                Text(meta.name)
                    .font(.body)
            }
            
            
            Spacer()
        }
        .padding(.vertical, 10)
        .padding(.horizontal)
        .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
    }
}
