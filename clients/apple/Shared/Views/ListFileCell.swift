import SwiftUI
import SwiftWorkspace
import Introspect

struct FileCell: View {
    let meta: File
    let isSelected: Bool
    let isSelectable: Bool
    
    init(meta: File, selectedFiles: Set<File>?) {
        self.meta = meta
        
        if let selectedFiles = selectedFiles {
            self.isSelectable = true
            self.isSelected = selectedFiles.contains(meta)
        } else {
            self.isSelectable = false
            self.isSelected = false
        }
    }

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
                    exportFilesAndShowShareSheet(metas: [meta])
                }, label: {
                    Label("Share externally to...", systemImage: "square.and.arrow.up.fill")
                })
                
                if meta.type == .document {
                    Button(action: {
                        DI.files.copyFileLink(id: meta.id)
                    }) {
                        Label("Copy file link", systemImage: "link")
                    }
                }
                
                Divider()
                
                Button(role: .destructive, action: {
                    DI.sheets.deleteConfirmationInfo = [meta]
                }) {
                    Label("Delete", systemImage: "trash.fill")
                }
            })
    }

    @ViewBuilder
    var cell: some View {
        Button(action: {
            if isSelectable {
                if isSelected {
                    withAnimation {
                        DI.selected.removeFileFromSelection(file: meta)
                    }
                } else {
                    withAnimation {
                        DI.selected.addFileToSelection(file: meta)
                    }
                }
            } else {
                if meta.type == .document {
                    DI.workspace.requestOpenDoc(meta.id)
                }
                
                DI.files.intoChildDirectory(meta)
            }
        }) {
            HStack(spacing: 20) {
                if isSelectable {
                    ZStack {
                        if isSelected {
                            Image(systemName: "circle.fill")
                                .foregroundStyle(.blue)
                                .font(.system(size: 17))
                        }
                        
                        Image(systemName: isSelected ? "checkmark" : "circle")
                            .foregroundStyle(isSelected ? Color.white : .secondary)
                            .font(.system(size: (isSelected ? 10 : 17)))
                    }
                }

                Image(systemName: FileService.metaToSystemImage(meta: meta))
                    .foregroundColor(meta.type == .folder ? .blue : .secondary)
                    .font(.title3)
                    .frame(width: 20)
                
                if meta.type == .document {
                    VStack(alignment: .leading) {
                        Text(meta.name)
                            .font(.body)
                            .lineLimit(1)
                        
                        Text(DI.core.getTimestampHumanString(timestamp: Int64(meta.lastModified)))
                                .foregroundColor(.secondary)
                                .font(.caption)
                    }
                } else {
                    Text(meta.name)
                        .font(.body)
                }
                
                Spacer()
            }
            .padding(.vertical, 13.5)
            .padding(.horizontal)
            .contentShape(Rectangle())
            .background(isSelected ? .gray.opacity(0.2) : .clear)
        }
    }
}
