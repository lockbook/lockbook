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
                if AppState.lb.events.status.outOfSpace {
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
                        homeState.showOutOfSpaceAlert = true
                    }
                }
                
                workspaceInput.requestSync()
            }
            .padding(.leading)
            .onChange(of: fileTreeModel.openDoc) { newValue in
                scrollHelper.scrollTo(newValue)
            }
            .onAppear {
                scrollHelper.scrollTo(fileTreeModel.openDoc)
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

struct FileRowContextMenu: View {
    let file: File
    
    @EnvironmentObject var fileTreeModel: FileTreeViewModel
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    
    var body: some View {
        VStack {
            if file.isFolder {
                Button(action: {
                    workspaceInput.createDocAt(parent: file.id, drawing: false)
                }) {
                    Label("Create a document", systemImage: "doc.fill")
                }
                Button(action: {
                    workspaceInput.createDocAt(parent: file.id, drawing: true)
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
                    filesModel.deleteFileConfirmation = [file]
                }) {
                    Label("Delete", systemImage: "trash.fill")
                }
            }
        }
    }
}

struct OpenDocModifier: ViewModifier {
    @Environment(\.colorScheme) var colorScheme
    @EnvironmentObject var fileTreeModel: FileTreeViewModel
    
    let file: File
        
    func body(content: Content) -> some View {
        if fileTreeModel.openDoc == file.id {
            content
                .foregroundColor(Color.white)
                .background(
                    RoundedRectangle(cornerRadius: 5, style: .continuous)
                        .foregroundStyle( Color.primary.opacity(colorScheme == .light ? 0.05 : 0.1))
                        .padding(.vertical, 2)
                        .padding(.trailing)
                )
        } else {
            content
        }
    }
}

#Preview {
    NavigationView {
        FileTreeView(root: (AppState.lb as! MockLb).file0, filesModel: FilesViewModel(), workspaceInput: WorkspaceInputState(), workspaceOutput: WorkspaceOutputState())
    }
    .environmentObject(HomeState(workspaceOutput: WorkspaceOutputState(), filesModel: FilesViewModel()))
    .environmentObject(FilesViewModel())
}
