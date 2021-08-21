import SwiftUI
import SwiftLockbookCore
import PencilKit

struct FileListView: View {
    @EnvironmentObject var core: GlobalState
    @State var creatingFile: Bool = false
    @State var creating: FileType?
    @State var creatingName: String = ""
    let currentFolder: ClientFileMetadata
    let account: Account
    @Binding var moving: ClientFileMetadata?
    @State var renaming: ClientFileMetadata?
    static var toolbar = ToolbarModel()
    @State private var selection: ClientFileMetadata?
    
    var files: [ClientFileMetadata] {
        core.files.filter {
            $0.parent == currentFolder.id && $0.id != currentFolder.id
        }
    }
    
    var body: some View {
        VStack {
            List (files) { meta in
                renderCell(meta: meta)
                    .popover(item: $moving, content: renderMoveDialog)
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
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    NavigationLink(
                        destination: SettingsView(settingsState: SettingsState(core: core), account: account).equatable()) {
                        Image(systemName: "gearshape.fill")
                            .foregroundColor(.blue)
                    }
                }
            }
            .onReceive(NotificationCenter.default.publisher(for: UIApplication.willResignActiveNotification)) { _ in
                core.syncing = true
            }
            .onReceive(NotificationCenter.default.publisher(for: UIApplication.willEnterForegroundNotification)) { _ in
                core.syncing = true
            }
            .sheet(isPresented: $creatingFile, content: {NewFileSheet(parent: currentFolder, onSuccess: fileSuccessfullyCreated)})
            .navigationBarTitle(currentFolder.name)
            HStack {
                BottomBar(core: core, onCreating: { creatingFile = true })
            }
            .padding(.horizontal, 10)
        }
    }
    
    func renderMoveDialog(meta: ClientFileMetadata) -> some View {
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
                                    core.updateFiles()
                                    core.checkForLocalWork()
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
    
    func renderCell(meta: ClientFileMetadata) -> AnyView {
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
                            core.updateFiles()
                            core.checkForLocalWork()
                        }
                    },
                    onCancel: {
                        renaming = nil
                        creatingName = ""
                    },
                    renaming: true
                )
            )
        } else {
            if meta.fileType == .Folder {
                return AnyView (
                    NavigationLink(
                        destination: FileListView(currentFolder: meta, account: account, moving: $moving), tag: meta, selection: $selection) {
                        FileCell(meta: meta)
                    }.isDetailLink(false)
                )
            } else {
                if meta.name.hasSuffix(".draw") {
                    let dl = DrawingLoader(model: core.openDrawing, toolbar: FileListView.toolbar, meta: meta, deleteChannel: core.deleteChannel)
                    return AnyView (NavigationLink(destination: dl.navigationBarTitle(meta.name, displayMode: .inline), tag: meta, selection: $selection) {
                        FileCell(meta: meta)
                    })
                } else {
                    let el = EditorLoader(content: core.openDocument, meta: meta, deleteChannel: core.deleteChannel)
                    return AnyView (NavigationLink(destination: el, tag: meta, selection: $selection) {
                        FileCell(meta: meta)
                    })
                }
            }
        }
    }
    
    func handleDelete(meta: ClientFileMetadata) {
        switch core.api.deleteFile(id: meta.id) {
        case .success(_):
            core.deleteChannel.send(meta)
            core.updateFiles()
            core.checkForLocalWork()
            selection = .none
        case .failure(let err):
            core.handleError(err)
        }
    }
    
    func fileSuccessfullyCreated(new: ClientFileMetadata) {
        creatingFile = false
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(500)) {
            selection = new
        }
    }
}

struct FileListView_Previews: PreviewProvider {
    static let core = GlobalState()
    
    static var previews: some View {
        NavigationView {
            FileListView(currentFolder: core.root!, account: core.account!, moving: .constant(.none))
        }
    }
}
