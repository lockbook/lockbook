import SwiftUI
import SwiftLockbookCore
import PencilKit

struct FileListView: View {
    @ObservedObject var core: GlobalState
    @State var showingAccount: Bool = false
    @State var creating: FileType?
    @State var creatingName: String = ""
    let currentFolder: FileMetadata
    let account: Account
    @Binding var moving: FileMetadata?
    @State var renaming: FileMetadata?
    static var toolbar = ToolbarModel()

    var files: [FileMetadata] {
        core.files.filter {
            $0.parent == currentFolder.id && $0.id != currentFolder.id
        }
    }
    
    var body: some View {
        ScrollView {
            VStack {
                creating.map { type in
                    SyntheticFileCell(
                        parent: currentFolder,
                        type: type,
                        name: $creatingName,
                        onCommit: {
                            handleCreate(meta: currentFolder, type: type)
                        },
                        onCancel: doneCreating,
                        renaming: false
                    )
                }

                ForEach(files) { meta in
                    renderCell(meta: meta)
                        .contextMenu(menuItems: {
                            Button(action: {
                                handleDelete(meta: meta)
                            }) {
                                Label("Delete", systemImage: "trash.fill")
                            }
                            Button(action: {
                                moving = meta
                            }, label: {
                                Label("Move", systemImage: "folder")
                            })
                            Button(action: {
                                renaming = meta
                                creatingName = meta.name
                            }, label: {
                                Label("Rename", systemImage: "pencil")
                            })
                        })

                }
            }
            .padding(.leading, 20)
        }
        .onReceive(NotificationCenter.default.publisher(for: UIApplication.willResignActiveNotification)) { _ in
            core.syncing = true
        }
        .onReceive(NotificationCenter.default.publisher(for: UIApplication.willEnterForegroundNotification)) { _ in
            core.syncing = true
        }
        .sheet(isPresented: $showingAccount, content: {
            AccountView(core: core, account: account)
        })
        .sheet(item: $moving, content: renderMoveDialog)
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

    func renderMoveDialog(meta: FileMetadata) -> some View {
        let root = core.files.first(where: { $0.parent == $0.id })!
        let wc = WithChild(root, core.files, { $0.id == $1.parent && $0.id != $1.id && $1.fileType == .Folder })
        
        return
            ScrollView {
                VStack {
                    Text("Moving \(meta.name)").font(.headline)
                    NestedList(
                        node: wc,
                        row: { dest in
                            Button(action: {
                                moving = nil
                                if case .failure(let err) = core.api.moveFile(id: meta.id, newParent: dest.id) {
                                    // Delaying this because the sheet has to go away before an alert can show up!
                                    DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(100)) {
                                        core.handleError(err)
                                    }
                                } else {
                                    withAnimation {
                                        core.updateFiles()
                                        core.checkForLocalWork()
                                    }
                                }
                            }, label: {
                                Label(dest.name, systemImage: "folder")
                            })
                        }
                    )
                    Spacer()
                }.padding()
            }
    }
    
    func renderCell(meta: FileMetadata) -> AnyView {
        if let isRenaming = renaming, isRenaming == meta {
            return AnyView(
                SyntheticFileCell(
                    parent: meta,
                    type: meta.fileType,
                    name: $creatingName,
                    onCommit: {
                        if case .failure(let err) = core.api.renameFile(id: meta.id, name: creatingName) {
                            core.handleError(err)
                        } else {
                            withAnimation {
                                core.updateFiles()
                                core.checkForLocalWork()
                            }
                        }
                    },
                    onCancel: {
                        withAnimation {
                            renaming = nil
                            creatingName = ""
                        }
                    },
                    renaming: true
                )
            )
        } else {
            if meta.fileType == .Folder {
                return AnyView (
                    NavigationLink(destination: FileListView(core: core, currentFolder: meta, account: account, moving: $moving)) {
                        FileCell(meta: meta)
                    }.isDetailLink(false)
                )
            } else {
                if meta.name.hasSuffix(".draw") {
                    // This is how you can pop without the navigation bar
                    // https://stackoverflow.com/questions/56513568/ios-swiftui-pop-or-dismiss-view-programmatically
                    let dl = DrawingLoader(model: core.openDrawing, toolbar: FileListView.toolbar, meta: meta)
                    return AnyView (NavigationLink(destination: dl.navigationBarTitle(meta.name, displayMode: .inline)) {
                        FileCell(meta: meta)
                    })
                } else {
                    let el = EditorLoader(content: core.openDocument, meta: meta, files: core.files)
                    return AnyView (NavigationLink(destination: el) {
                        FileCell(meta: meta)
                    })
                }
            }
        }
    }

    func handleDelete(meta: FileMetadata) {
        switch core.api.deleteFile(id: meta.id) {
        case .success(_):
            core.updateFiles()
            core.checkForLocalWork()
        case .failure(let err):
            core.handleError(err)
        }
    }
    
    func handleCreate(meta: FileMetadata, type: FileType) {
        switch core.api.createFile(name: creatingName, dirId: meta.id, isFolder: type == .Folder) {
        case .success(_):
            doneCreating()
            core.updateFiles()
            core.checkForLocalWork()
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
            creatingName = ".md"
        }
    }
    
    func newDrawing() {
        withAnimation {
            creating = .Document
            creatingName = ".draw"
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
            FileListView(core: core, showingAccount: false, currentFolder: core.root!, account: core.account!, moving: .constant(.none))
        }
    }
}
