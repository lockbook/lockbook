import SwiftUI
import SwiftLockbookCore

struct FileListView: View {
    @ObservedObject var core: Core
    @State var showingAccount: Bool = false
    @State var creating: FileType?
    @State var creatingName: String = ""
    let currentFolder: FileMetadata
    let account: Account
    
    var files: [FileMetadata] {
        core.files.filter {
            $0.parent == currentFolder.id && $0.id != currentFolder.id
        }
    }
    
    var body: some View {
        ScrollView {
            VStack {
                creating.map { type in
                    SyntheticFileCell(params: (currentFolder, type), nameField: $creatingName, onCreate: {
                        handleCreate(meta: currentFolder, type: type)
                    }, onCancel: doneCreating)
                }
                
                ForEach(files) { meta in
                    renderCell(meta: meta)
                        .contextMenu(menuItems: {
                            Button(action: {
                                handleDelete(meta: meta)
                            }) {
                                Label("Delete", systemImage: "trash.fill")
                            }
                        })
                    
                }
            }
            .padding(.leading, 20)
        }
        
        .sheet(isPresented: $showingAccount, content: {
            AccountView(core: core, account: account)
        })
        .toolbar {
            ToolbarItem(placement: .navigationBarTrailing) {
                Button(action: { showingAccount.toggle() }) {
                    Image(systemName: "gearshape.fill")
                }
            }
            ToolbarItemGroup(placement: .bottomBar) {
                BottomBar(core: core, onNewDocument: newDocument, onNewFolder: newFolder)
            }
        }
        .navigationBarTitle(currentFolder.name)
        
    }
    
    func renderCell(meta: FileMetadata) -> AnyView {
        if meta.fileType == .Folder {
            return AnyView (
                NavigationLink(destination: FileListView(core: core, currentFolder: meta, account: account)) {
                    FileCell(meta: meta)
                }.isDetailLink(false)
            )
        } else {
            return AnyView (NavigationLink(destination: EditorLoader(core: core, meta: meta)) {
                FileCell(meta: meta)
                
            })
        }
    }

    func handleDelete(meta: FileMetadata) {
        switch core.api.deleteFile(id: meta.id) {
        case .success(_):
            core.updateFiles()
        case .failure(let err):
            core.handleError(err)
        }
    }
    
    func handleCreate(meta: FileMetadata, type: FileType) {
        switch core.api.createFile(name: creatingName, dirId: meta.id, isFolder: type == .Folder) {
        case .success(_):
            doneCreating()
            core.updateFiles()
        case .failure(let err):
            core.handleError(err)
        }
    }
    
    func doneCreating() {
        withAnimation {
            creating = .none
            creatingName = ""
        }
    }
    
    func newDocument() {
        withAnimation {
            creating = .Document
            creatingName = ""
        }
    }
    
    func newFolder() {
        withAnimation {
            creating = .Folder
            creatingName = ""
        }
    }
}

struct FileListView_Previews: PreviewProvider {
    static let core = Core()
    
    static var previews: some View {
        NavigationView {
            FileListView(core: core,
                         showingAccount: false, currentFolder: core.root!,
                         account: core.account!)
        }
    }
}
