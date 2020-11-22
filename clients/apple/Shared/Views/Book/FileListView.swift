import SwiftUI
import SwiftLockbookCore

struct FileListView: View {
    @ObservedObject var core: Core
    let account: Account
    let root: FileMetadata
    @State var showingAccount: Bool = false
    @State var creating: FileType?
    @State var creatingName: String = ""
    @State var currentFolder: FileMetadata

    var body: some View {
        let filtered = core.files.filter {
            $0.parent == currentFolder.id && $0.id != currentFolder.id
        }
        let baseView = List {
            HStack {
                Button(action: {
                    selectFolder(meta: core.files.first(where: { $0.id == currentFolder.parent })!)
                }) {
                    Image(systemName: "arrow.turn.left.up")
                }
                .foregroundColor(.accentColor)
                Text(currentFolder.name)
            }
            creating.map { creatingType in
                SyntheticFileCell(params: (currentFolder, creatingType), nameField: $creatingName, onCreate: {
                    handleCreate(meta: currentFolder, type: creatingType)
                }, onCancel: doneCreating)
            }
            ForEach(filtered) { meta in
                renderCell(meta: meta)
            }
            .onDelete(perform: {
                handleDelete(meta: filtered[$0.first!])
            })
            HStack {
                Spacer()
                Text("\(core.files.count) items")
                    .foregroundColor(.secondary)
                Spacer()
            }
        }
        .onReceive(core.timer, perform: { _ in
            core.sync()
        })

        #if os(iOS)
        return baseView
            .sheet(isPresented: $showingAccount, content: {
                AccountView(core: core, account: account)
            })
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button(action: { showingAccount.toggle() }) {
                        Image(systemName: "person.circle.fill")
                    }
                }
                ToolbarItem(placement: .navigationBarLeading) {
                    Button(action: { creating = .Folder }) {
                        Image(systemName: "folder.fill.badge.plus")
                    }
                }
                ToolbarItem(placement: .navigationBarLeading) {
                    Button(action: { creating = .Document }) {
                        Image(systemName: "doc.on.doc.fill")
                    }
                }
            }
        #else
        return baseView
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button(action: { creating = .Folder }) {
                        Image(systemName: "folder.fill.badge.plus")
                    }
                }
                ToolbarItem(placement: .primaryAction) {
                    Button(action: { creating = .Document }) {
                        Image(systemName: "doc.on.doc.fill")
                    }
                }
            }
        #endif
    }

    init(core: Core, account: Account, root: FileMetadata) {
        self.core = core
        self.account = account
        self.root = root
        self._currentFolder = .init(initialValue: root)
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

    func handleDelete(meta: FileMetadata) {
        switch core.api.deleteFile(id: meta.id) {
        case .success(_):
            core.updateFiles()
        case .failure(let err):
            core.handleError(err)
        }
    }

    func selectFolder(meta: FileMetadata) {
        withAnimation {
            currentFolder = meta
        }
    }

    func doneCreating() {
        withAnimation {
            creating = .none
            creatingName = ""
        }
    }

    func renderCell(meta: FileMetadata) -> AnyView {
        if meta.fileType == .Folder {
            return AnyView(
                Button(action: { selectFolder(meta: meta) }) {
                    FileCell(meta: meta)
                }
            )
        } else {
            return AnyView(
                NavigationLink(destination: EditorView(core: core, meta: meta).equatable()) {
                    FileCell(meta: meta)
                }
            )
        }
    }
}

struct FileListView_Previews: PreviewProvider {
    static let core = Core()

    static var previews: some View {
        NavigationView {
            FileListView(core: core, account: core.account!, root: core.root!)
        }
    }
}
