import SwiftUI
import SwiftWorkspace

struct FileTreeView: View {
    
    @EnvironmentObject var workspaceState: WorkspaceState
    @StateObject var fileTreeModel: FileTreeViewModel
    
    @Environment(\.isConstrainedLayout) var isConstrainedLayout
    
    var root: File
    
    init(root: File, workspaceState: WorkspaceState, filesModel: FilesViewModel) {
        self.root = root
        self._fileTreeModel = StateObject(wrappedValue: FileTreeViewModel(workspaceState: workspaceState, filesModel: filesModel))
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 2) {
                FileRowView(file: root, level: -1)
                    .environmentObject(fileTreeModel)
                
                Spacer()
            }
            .listStyle(.sidebar)
            .frame(minWidth: 10, maxWidth: .infinity, maxHeight: .infinity)
        }.contextMenu {
            FileRowContextMenu(file: root)
        }
        .refreshable {
            workspaceState.requestSync()
        }
        .padding(.leading)
        .toolbar {
            switch fileTreeModel.selectedFilesState {
            case .selected(explicitly: _, implicitly: _):
                ToolbarItem(placement: .topBarTrailing) {
                    Button(action: {
                        withAnimation {
                            fileTreeModel.selectedFilesState = .unselected
                        }
                    }, label: {
                        Text("Done")
                            .foregroundStyle(.blue)
                    })
                }
            case .unselected:
                ToolbarItemGroup(placement: .topBarTrailing) {
                    Button(action: {
                        withAnimation(.linear(duration: 0.2)) {
                            fileTreeModel.selectedFilesState = .selected(explicitly: [], implicitly: [])
                        }
                    }, label: {
                        Text("Edit")
                            .foregroundStyle(.blue)
                    })
                }
            }
            
        }
    }
}

#Preview {
    NavigationView {
        FileTreeView(root: (AppState.lb as! MockLb).file0, workspaceState: WorkspaceState(), filesModel: FilesViewModel(workspaceState: WorkspaceState()))
    }
    .environmentObject(HomeState())
    .environmentObject(FilesViewModel(workspaceState: WorkspaceState()))
    .environmentObject(WorkspaceState())
}

struct FileRowView: View {
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var fileTreeModel: FileTreeViewModel
    @EnvironmentObject var workspaceState: WorkspaceState
    
    let file: File
    let level: CGFloat
    
    var children: [File] {
        get {
            filesModel.childrens[file.id] ?? []
        }
    }

    var isLeaf: Bool {
        get {
            children.isEmpty
        }
    }
        
    var isOpen: Bool {
        get {
            fileTreeModel.openFolders.contains(file.id) || fileTreeModel.implicitlyOpenFolders.contains(file.id)
        }
    }
    
    var isSelected: Bool {
        fileTreeModel.selectedFilesState.isSelected(file)
    }
    
    var isSelectable: Bool {
        fileTreeModel.selectedFilesState.isSelectableState()
    }
    
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if level != -1 {
                fileRow
                    .onTapGesture {
                        self.openOrSelectFile()
                    }
            }
            
            if !isLeaf && (isOpen || level == -1) {
                ForEach(children) { child in
                    FileRowView(file: child, level: level + 1)
                }
            }
        }
        .contextMenu {
            FileRowContextMenu(file: file)
        }
    }
    
    var fileRow: some View {
        HStack {
            if isSelectable {
                ZStack {
                    if isSelected {
                        Image(systemName: "circle.fill")
                            .foregroundStyle(.blue)
                            .font(.system(size: 17))
                    }
                    
                    Image(systemName: isSelected ? "checkmark" : "circle")
                        .foregroundStyle(isSelected ? Color.white : Color.secondary)
                        .font(.system(size: (isSelected ? 10 : 17)))
                }
                .padding(.trailing, 5)
            }
            
            Image(systemName: FileIconHelper.fileToSystemImageName(file: file))
                .font(.system(size: 16))
                .frame(width: 16)
                .foregroundColor(file.isFolder ? .accentColor : (false ? .white : .secondary ))
                        
            Text(file.name)
                .lineLimit(1)
                .truncationMode(.tail)
                .allowsTightening(true)
            
            Spacer()
            
            if !isLeaf {
                Image(systemName: "chevron.forward")
                    .renderingMode(.template)
                    .resizable()
                    .scaledToFit()
                    .frame(width: 10, height: 10)
                    .rotationEffect(Angle.degrees(isOpen ? 90 : 0))
                    .foregroundColor(.accentColor)
            }

        }
        .padding(.vertical, 9)
        .contentShape(Rectangle())
        .padding(.leading, level * 20 + 5)
        .padding(.trailing, 10)
    }
    
    func openOrSelectFile() {
        if isSelectable {
            if isSelected {
                fileTreeModel.removeFileFromSelection(file: file)
            } else {
                fileTreeModel.addFileToSelection(file: file)
            }
        }
        
        if file.isFolder {
            workspaceState.selectedFolder = file.id
            
            withAnimation {
                let _ = fileTreeModel.openFolders.insert(file.id)
            }
        } else {
            workspaceState.requestOpenDoc(file.id)
        }
    }
}

struct FileRowContextMenu: View {
    let file: File
    
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var homeState: HomeState
    
    var body: some View {
        VStack {
            if file.isFolder {
                Button(action: {
                    filesModel.createDoc(parent: file.id, isDrawing: false)
                }) {
                    Label("Create a document", systemImage: "doc.fill")
                }
                Button(action: {
                    filesModel.createDoc(parent: file.id, isDrawing: true)
                }) {
                    Label("Create a drawing", systemImage: "pencil.tip.crop.circle.badge.plus")
                }
                Button(action: {
                    homeState.sheetInfo = .createFolder(parent: file)
                }) {
                    Label("Create a folder", systemImage: "folder.fill")
                }
            }
            
            if !file.isRoot {
                Button(action: {
                    homeState.sheetInfo = .rename(file: file)
                }) {
                    Label("Rename", systemImage: "pencil.circle.fill")
                }

                Button(action: {
                    homeState.selectSheetInfo = .move(files: [file])
                }) {
                    Label("Move", systemImage: "arrow.up.and.down.and.arrow.left.and.right")
                }
                
                Divider()
                
                Button(action: {
                    homeState.sheetInfo = .share(file: file)
                }) {
                    Label("Share", systemImage: "person.wave.2.fill")
                }

                Button(action: {
                    exportFiles(homeState: homeState, files: [file])
                }) {
                    Label("Share externally to...", systemImage: "square.and.arrow.up.fill")
                }
                
                if file.type == .document {
                    Button(action: {
                        ClipboardHelper.copyFileLink(file.id)
                    }) {
                        Label("Copy file link", systemImage: "link")
                    }
                }
                
                Divider()
                
                Button(role: .destructive, action: {
//                    DI.sheets.deleteConfirmationInfo = [meta]
                }) {
                    Label("Delete", systemImage: "trash.fill")
                }
            }
        }
    }
}
