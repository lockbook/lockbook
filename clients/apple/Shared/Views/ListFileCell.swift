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
    let matchedIndices: [Int]
    
    @State var nameModified: Text = Text("")
    @State var pathModified: Text = Text("")
    
    @State var modifiedGenerated = false
    
    @Environment(\.horizontalSizeClass) var horizontal
    @Environment(\.verticalSizeClass) var vertical
    
    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            nameModified
                .font(.title3)
            
            HStack {
                Image(systemName: "doc")
                    .foregroundColor(.accentColor)
                    .font(.caption)
                
                pathModified
                        .foregroundColor(.accentColor)
                        .font(.caption)
            }
        }
            .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
            .iOSSearchPadding(horizontal: horizontal, vertical: vertical)
            .onAppear {
                underlineMatchedSegments()
            }
    }
    
    func underlineMatchedSegments() {
        if(modifiedGenerated) {
            return
        }
        
        modifiedGenerated = true
        
        let matchedIndicesHash = Set(matchedIndices)
        
        var pathOffset = 1;
        
        if(path.count - 1 > 0) {
            pathModified = Text("")
            
            for index in 0...path.count - 1 {
                let correctIndex = String.Index(utf16Offset: index, in: path)
                let newPart = Text(path[correctIndex...correctIndex])
                                
                if(path[correctIndex...correctIndex] == "/") {
                    pathModified = pathModified + Text(" > ").foregroundColor(.gray)
                } else if(matchedIndicesHash.contains(index + 1)) {
                    pathModified = pathModified + newPart.bold()
                } else {
                    pathModified = pathModified + newPart
                }
            }
            
            pathOffset = 2
        }
                
        if(name.count - 1 > 0) {
            nameModified = Text("")
            for index in 0...name.count - 1 {
                let correctIndex = String.Index(utf16Offset: index, in: name)
                let newPart = Text(name[correctIndex...correctIndex])
                
                if(matchedIndicesHash.contains(index + path.count + pathOffset)) {
                    nameModified = nameModified + newPart.bold()
                } else {
                    nameModified = nameModified + newPart.foregroundColor(.gray)
                }
            }
        }
    }
}

struct SearchFileContentCell: View {
    let name: String
    let path: String
    let paragraph: String
    let matchedIndices: [Int]
    
    @State var paragraphModified: Text = Text("")
    @State var pathModified: Text = Text("")
    
    @State var modifiedGenerated = false
    
    @Environment(\.horizontalSizeClass) var horizontal
    @Environment(\.verticalSizeClass) var vertical

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(name)
                .font(.title3)
                .foregroundColor(.gray)
            
            HStack {
                Image(systemName: "doc")
                    .foregroundColor(.accentColor)
                    .font(.caption2)
                
                pathModified
                        .foregroundColor(.accentColor)
                        .font(.caption2)
            }
            .padding(.bottom)
            
            paragraphModified
                .font(.caption)
        }
            .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
            .iOSSearchPadding(horizontal: horizontal, vertical: vertical)
            .onAppear {
                underlineMatchedSegments()
            }
    }
    
    func underlineMatchedSegments() {
        if(modifiedGenerated) {
            return
        }
        
        modifiedGenerated = true

        let matchedIndicesHash = Set(matchedIndices)
        
        if(path.count - 1 > 0) {
            pathModified = Text("")
            
            for index in 0...path.count - 1 {
                let correctIndex = String.Index(utf16Offset: index, in: path)
                                
                if(path[correctIndex...correctIndex] == "/") {
                    pathModified = pathModified + Text(" > ").foregroundColor(.gray)
                } else {
                    pathModified = pathModified + Text(path[correctIndex...correctIndex])
                }
            }
        }
                
        if(paragraph.count - 1 > 0) {
            paragraphModified = Text("")
            
            for index in 0...paragraph.count - 1 {
                let correctIndex = String.Index(utf16Offset: index, in: paragraph)
                let newPart = Text(paragraph[correctIndex...correctIndex])
                
                if(matchedIndicesHash.contains(index)) {
                    paragraphModified = paragraphModified + newPart.bold()
                } else {
                    paragraphModified = paragraphModified + newPart.foregroundColor(.gray)
                }
            }
            
        }
    }
}

extension View {
    public func iOSSearchPadding(horizontal: UserInterfaceSizeClass?, vertical: UserInterfaceSizeClass?) -> some View {
        Group {
            if horizontal == .regular && vertical == .regular {
                self
            } else {
                self
                    .padding(.vertical, 5)
            }
        }
    }
}
