///
/// Ripped from: https://github.com/toph-allen/OutlineView/blob/main/OutlineView/OutlineView.swift
///
import SwiftUI
import Combine
import SwiftLockbookCore

struct OutlineBranch: View {
    @ObservedObject var core: Core
    
    var file: FileMetadata
    @Binding var selectedItem: FileMetadata?
    var level: CGFloat
    @State var open: Bool = false
    
    var children: [FileMetadata] {
        core.files.filter {
            $0.parent == file.id && $0.id != file.id
        }
    }
    
    var isLeaf: Bool {
        children.isEmpty
    }
    
    @ViewBuilder
    var body: some View {
        VStack(spacing: 2) { // spacing: 2 is what List uses
            if level == -1 {
                EmptyView() // the root node is at
            } else {
                
                if file == selectedItem {
                    OutlineRow(core: core, file: file, level: level, open: $open)
                        .background(Color.accentColor)
                        .foregroundColor(Color.white)
                        .clipShape(RoundedRectangle(cornerRadius: 5, style: .continuous))
                } else {
                    OutlineRow(core: core, file: file, level: level, open: $open)
                        .onTapGesture {
                            if file.fileType == .Folder {
                                self.open.toggle()
                            } else {
                                self.selectedItem = self.file
                            }
                        }
                }
            }
            if isLeaf == false && (open == true || level == -1) {
                ForEach(children) { child in
                    OutlineBranch(core: core, file: child, selectedItem: self.$selectedItem, level: self.level + 1)
                }
            }
        }
    }
}


struct OutlineSection: View {
    
    @ObservedObject var core: Core
    
    var root: FileMetadata
    @Binding var selectedItem: FileMetadata?
    @State var creating: FileType?
    
    var children: [FileMetadata] {
        core.files.filter {
            $0.parent == root.id && $0.id != root.id
        }
    }
    
    var isLeaf: Bool {
        children.isEmpty
    }
    
    var body: some View {
        List {
            // The padding in the section header is there to adjust for the inset hack.
            Section(header: Text(root.name).padding(.leading, 8)) {
                OutlineBranch(core: core, file: root, selectedItem: self.$selectedItem, level: -1)
            }
            .collapsible(false)
            creating.map { c in
                SyntheticOutlineRow(fileType: c, level: 0, onCreate: handleCreate(meta: root, type: c))
            }
        }
        .listStyle(SidebarListStyle())
        .frame(minWidth: 10, maxWidth: .infinity, maxHeight: .infinity)
        .padding(.leading, -8)
        .contextMenu(menuItems: {
            makeContextActions(parent: root, creating: $creating)
        })
        // A hack for list row insets not working. This hack also applies to the section header though.
    }

    func handleCreate(meta: FileMetadata, type: FileType) -> (String) -> Void {
        return { creatingName in
            switch core.api.createFile(name: creatingName, dirId: meta.id, isFolder: type == .Folder) {
            case .success(_):
                doneCreating()
                core.updateFiles()
            case .failure(let err):
                core.handleError(err)
            }
        }
    }

    func doneCreating() {
        withAnimation {
            creating = .none
        }
    }
}

func makeContextActions(parent: FileMetadata, creating: Binding<FileType?>) -> TupleView<(Text, Button<Label<Text, Image>>, Button<Label<Text, Image>>)> {
    TupleView((
        Text("\(parent.name)::\(parent.fileType.rawValue)"),
        Button(action: { creating.wrappedValue = .Document }) {
            Label("Create a document", systemImage: "doc")
        },
        Button(action: { creating.wrappedValue = .Folder }) {
            Label("Create a folder", systemImage: "folder")
        }
    ))
}

