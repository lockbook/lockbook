import SwiftUI
import SwiftLockbookCore
import Introspect

struct FileCell: View {
    let meta: File

    @EnvironmentObject var current: CurrentDocument
    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var account: AccountService

    var body: some View {
        cell
                .contextMenu(menuItems: {
                    // TODO: disast: https://stackoverflow.com/questions/70159437/context-menu-not-updating-in-swiftui
                    Button(action: handleDelete) {
                        Label("Delete", systemImage: "trash.fill")
                    }
                    Button(action: {
                        sheets.movingInfo = meta
                    }, label: {
                        Label("Move", systemImage: "arrow.up.and.down.and.arrow.left.and.right")
                    })
                    Button(action: {
                        sheets.renamingInfo = meta
                    }, label: {
                        Label("Rename", systemImage: "questionmark.folder")
                    })
                    Button(action: {
                        sheets.sharingFileInfo = meta
                    }, label: {
                        Label("Share", systemImage: "shareplay")
                    })
                })
    }

    @ViewBuilder
    var cell: some View {
        if meta.fileType == .Folder {
            Button(action: {
                fileService.intoChildDirectory(meta)
            }) {
                RealFileCell(meta: meta)
            }
        } else {
            NavigationLink(destination: DocumentView(meta: meta)) {
                RealFileCell(meta: meta)
            }
        }
    }

    func handleDelete() {
        fileService.deleteFile(id: meta.id)
    }
}

struct RealFileCell: View {
    let meta: File

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(meta.name)
                    .font(.title3)
            HStack {
                Image(systemName: meta.fileType == .Folder ? "folder.fill" : "doc.fill")
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

struct SearchFilePathCell: View {
    let name: String
    let path: String
    
    @State var formattedName: Text
    @State var formattedPath: Text
    
    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(.init(name))
                .font(.title3)
            
            HStack {
                Image(systemName: "doc")
                    .foregroundColor(.accentColor)
                    .font(.caption)
                
                Text(.init(path))
                        .foregroundColor(.accentColor)
                        .font(.caption)
            }
        }
            .padding(.vertical, 5)
            .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
    }
    
//    func formatPathAndName() {
//        var tempFormattedName = Text("")
//        var tempFormattedPath = Text("")
//        
//        for index in (0...matchedIndices.count - 1).reversed() {
//            let correctIndex = formattedFullPath.index(formattedFullPath.startIndex, offsetBy: matchedIndices[index])
//            
//            formattedFullPath.replaceSubrange(correctIndex...correctIndex, with: "**\(formattedFullPath[correctIndex])**")
//        }
//
//    }
}

struct SearchFileContentCell: View {
    let name: String
    let path: String
    let paragraph: String

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(name)
                .font(.title3)
            
            HStack {
                Image(systemName: "doc")
                    .foregroundColor(.accentColor)
                    .font(.caption2)
                
                Text(path)
                        .foregroundColor(.accentColor)
                        .font(.caption2)
            }
            .padding(.bottom)
            
            Text(.init(paragraph))
                .font(.caption)
        }
            .padding(.vertical, 5)
            .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
    }
}

