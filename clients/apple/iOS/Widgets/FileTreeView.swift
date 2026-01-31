import SwiftUI
import SwiftWorkspace

struct FileTreeView: View {
    
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    
    @StateObject var fileTreeModel: FileTreeViewModel
    
    var root: File
    
    init(root: File, filesModel: FilesViewModel, workspaceInput: WorkspaceInputState, workspaceOutput: WorkspaceOutputState) {
        self.root = root
        self._fileTreeModel = StateObject(wrappedValue: FileTreeViewModel(filesModel: filesModel, workspaceInput: workspaceInput, workspaceOutput: workspaceOutput))
    }

    var body: some View {
        ScrollViewReader { scrollHelper in
            Group {
                VStack(alignment: .leading, spacing: 2) {
                    FileRowView(file: root, level: -1)
                        .environmentObject(fileTreeModel)
                    
                    Spacer()
                }
                .listStyle(.sidebar)
                .padding(.leading)
                .onChange(of: fileTreeModel.openDoc) { newValue in
                    scrollHelper.scrollTo(newValue)
                }
                .onAppear {
                    scrollHelper.scrollTo(fileTreeModel.openDoc)
                }
                
                Spacer().frame(height: 150)
            }
            .contentShape(Rectangle())
            .contextMenu {
                FileRowContextMenu(file: root)
            }
        }
    }
}

struct FileRowView: View {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var fileTreeModel: FileTreeViewModel
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    
    @Environment(\.horizontalSizeClass) var horizontalSizeClass

    let file: File
    let level: CGFloat
    
    var children: [File] {
        get {
            (filesModel.childrens[file.id] ?? []).sorted { $1 > $0 }
        }
    }

    var isLeaf: Bool {
        get {
            children.isEmpty
        }
    }
        
    var isOpen: Bool {
        get {
            fileTreeModel.openFolders.contains(file.id)
        }
    }
    
    var isSelected: Bool {
        filesModel.selectedFilesState.isSelected(file)
    }
    
    var isSelectable: Bool {
        filesModel.selectedFilesState.isSelectableState()
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
                ForEach(children, id: \.id) { child in
                    FileRowView(file: child, level: level + 1)
                }
            }
        }
        .contextMenu {
            FileRowContextMenu(file: file)
        }
        .id(file.id)
    }
    
    var fileRow: some View {
        HStack {
            if isSelectable {
                ZStack {
                    if isSelected {
                        Image(systemName: "circle.fill")
                            .foregroundStyle(Color.accentColor)
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
                .foregroundColor(file.isFolder ? .accentColor : .secondary)
                        
            Text(file.name)
                .lineLimit(1)
                .truncationMode(.tail)
                .allowsTightening(true)
                .foregroundColor(.primary)
            
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
        .modifier(OpenDocModifier(file: file))
        .confirmationDialog(
            "Are you sure? This action cannot be undone.",
            isPresented: Binding(
                get: { filesModel.isFileInDeletion(id: file.id) },
                set: { _, _ in filesModel.deleteFileConfirmation = nil }
            ),
            titleVisibility: .visible, actions: {
                if let files = filesModel.deleteFileConfirmation {
                    DeleteConfirmationButtons(files: files)
                }
            }
        )
    }
    
    func openOrSelectFile() {
        homeState.closeWorkspaceBlockingScreens()
        
        if isSelectable {
            if isSelected {
                filesModel.removeFileFromSelection(file: file)
            } else {
                filesModel.addFileToSelection(file: file)
            }
            
            return
        }
        
        if file.isFolder {
            fileTreeModel.supressNextOpenFolder = true
            workspaceInput.selectFolder(id: file.id)
            
            withAnimation {
                let _ = fileTreeModel.toggleFolder(file.id)
            }
        } else {
            workspaceInput.openFile(id: file.id)
            
            if homeState.isSidebarFloating {
                homeState.sidebarState = .closed
            }
        }
    }
}

#Preview {
    NavigationView {
        FileTreeView(root: (AppState.lb as! MockLb).file0, filesModel: .preview, workspaceInput: .preview, workspaceOutput: .preview)
    }
    .withCommonPreviewEnvironment()
}
