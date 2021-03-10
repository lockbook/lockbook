import SwiftUI
import SwiftLockbookCore
import PencilKit

struct FileListView: View {
    @ObservedObject var core: GlobalState
    @State var showingAccount: Bool = false
    @State var creating: FileType?
    @State var creatingName: String = ""
    @State var creatingFileExtension = ""
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
                    SyntheticFileCell(parent: currentFolder, type: type, nameField: $creatingName, fileExtension: $creatingFileExtension, onCreate: {
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
                BottomBar(core: core, onNewDocument: newDocument, onNewDrawing: newDrawing, onNewFolder: newFolder)
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
            if meta.name.hasSuffix(".draw") {
                // This is how you can pop without the navigation bar
                // https://stackoverflow.com/questions/56513568/ios-swiftui-pop-or-dismiss-view-programmatically
                return AnyView (NavigationLink(destination: DrawingLoader(model: DrawingModel(core: core, meta: meta), toolbar: ToolbarModel()).navigationBarTitle(meta.name, displayMode: .inline)) {
                    FileCell(meta: meta)
                })
            } else {
                return AnyView (NavigationLink(destination: EditorLoader(core: core, meta: meta)) {
                    FileCell(meta: meta)
                })
            }
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
        switch core.api.createFile(name: creatingName + creatingFileExtension, dirId: meta.id, isFolder: type == .Folder) {
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
            creatingFileExtension = ".md"
        }
    }
    
    func newDrawing() {
        withAnimation {
            creating = .Document
            creatingName = ""
            creatingFileExtension = ".draw"
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
    static let core = GlobalState()
    
    static var previews: some View {
        NavigationView {
            FileListView(core: core,
                         showingAccount: false, currentFolder: core.root!,
                         account: core.account!)
        }
    }
}
