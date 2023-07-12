import SwiftUI
import SwiftLockbookCore
import Introspect

struct FileCell: View {
    let meta: File

    var body: some View {
        cell
            .contextMenu(menuItems: {
                // TODO: disast: https://stackoverflow.com/questions/70159437/context-menu-not-updating-in-swiftui
                Button(action: {
                    DI.files.deleteFile(id: meta.id)
                }) {
                    Label("Delete", systemImage: "trash.fill")
                }
                Button(action: {
                    DI.sheets.movingInfo = meta
                }, label: {
                    Label("Move", systemImage: "arrow.up.and.down.and.arrow.left.and.right")
                })
                Button(action: {
                    DI.sheets.sharingFileInfo = meta
                }, label: {
                    Label("Share", systemImage: "shareplay")
                })
            })
    }

    @ViewBuilder
    var cell: some View {
        if meta.fileType == .Folder {
            Button(action: {
                DI.files.intoChildDirectory(meta)
            }) {
                RealFileCell(meta: meta)
            }
        } else {
            NavigationLink(destination: iOSDocumentViewWrapper(id: meta.id)) {
                RealFileCell(meta: meta)
            }
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
