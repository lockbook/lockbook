import SwiftUI
import SwiftLockbookCore
import PencilKit

struct FileListView: View {
    
    @EnvironmentObject var coreService: CoreService
    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var sync: SyncService
    @EnvironmentObject var status: StatusService
    @EnvironmentObject var errors: UnexpectedErrorService
    @EnvironmentObject var onboarding: OnboardingService
    
    @State var creatingFile: Bool = false
    @State var creating: FileType?
    @State var creatingName: String = ""
    let currentFolder: DecryptedFileMetadata
    let account: Account
    @Binding var moving: DecryptedFileMetadata?
    @State var renaming: DecryptedFileMetadata?
    @State private var selection: DecryptedFileMetadata?
    @State private var newFile: DecryptedFileMetadata?
    
    var files: [DecryptedFileMetadata] {
        fileService.files.filter {
            $0.parent == currentFolder.id && $0.id != currentFolder.id
        }
    }
    
    var body: some View {
        ZStack {
            // The whole active selection concept doesn't handle links that don't exist yet properly
            // This is a workaround for that scenario.
            // This doesn't highlight properly on iPad, maybe yet another reason to roll our own navigation on ipad.
            if let newDoc = newFile, newDoc.fileType == .Document {
                NavigationLink(destination: DocumentView(meta: newDoc), isActive: Binding.constant(true)) {
                    EmptyView()
                }.hidden()
            }
            
            if let newFolder = newFile, newFolder.fileType == .Folder {
                NavigationLink(
                    destination: FileListView(currentFolder: newFolder, account: account, moving: $moving), isActive: Binding.constant(true)) {
                        EmptyView()
                    }.isDetailLink(false)
                    .hidden()
            }
            
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
                                creatingName = meta.decryptedName
                            }, label: {
                                Label("Rename", systemImage: "pencil")
                            })
                        })
                }
                .toolbar {
                    ToolbarItem(placement: .navigationBarTrailing) {
                        NavigationLink(
                            destination: SettingsView(account: account).equatable(), isActive: $onboarding.theyChoseToBackup) {
                                Image(systemName: "gearshape.fill")
                                    .foregroundColor(.blue)
                            }
                    }
                }
                .onReceive(NotificationCenter.default.publisher(for: UIApplication.willResignActiveNotification)) { _ in
                    sync.sync()
                }
                .onReceive(NotificationCenter.default.publisher(for: UIApplication.willEnterForegroundNotification)) { _ in
                    sync.sync()
                }
                .navigationBarTitle(currentFolder.decryptedName)
                HStack {
                    BottomBar(onCreating: { creatingFile = true })
                }
                .padding(.horizontal, 10)
                .sheet(isPresented: $onboarding.anAccountWasCreatedThisSession, content: { BeforeYouStart() })
                .sheet(isPresented: $creatingFile, onDismiss: {
                    self.selection = self.newFile
                }, content: {
                    NewFileSheet(parent: currentFolder, showing: $creatingFile, selection: $newFile)
                })
                .onChange(of: selection) {_ in
                    // When we return back to this screen, we have to change newFile back to nil regardless
                    // of it's present value, otherwise we won't be able to navigate to new, new files
                    if self.selection == nil { self.newFile = nil }
                }
            }
        }
        
    }
    
    func renderMoveDialog(meta: DecryptedFileMetadata) -> some View {
        let root = fileService.files.first(where: { $0.parent == $0.id })!
        let wc = WithChild(root, fileService.files, { $0.id == $1.parent && $0.id != $1.id && $1.fileType == .Folder })
        
        return ScrollView {
            VStack {
                Text("Moving \(meta.decryptedName)").font(.headline)
                NestedList(
                    node: wc,
                    row: { dest in
                        Button(action: {
                            moving = nil
                            fileService.moveFile(id: meta.id, newParent: dest.id)
                        }, label: {
                            Label(dest.decryptedName, systemImage: "folder")
                        })
                    }
                )
                Spacer()
            }.padding()
        }
    }
    
    func renderCell(meta: DecryptedFileMetadata) -> AnyView {
        if let isRenaming = renaming, isRenaming == meta {
            return AnyView(
                SyntheticFileCell(
                    parent: meta,
                    type: meta.fileType,
                    name: $creatingName,
                    onCommit: {
                        fileService.renameFile(id: meta.id, name: creatingName)
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
                let el = DocumentView(meta: meta)
                return AnyView (NavigationLink(destination: el, tag: meta, selection: $selection) {
                    FileCell(meta: meta)
                })
            }
        }
    }
    
    func handleDelete(meta: DecryptedFileMetadata) {
        self.fileService.deleteFile(id: meta.id)
        selection = .none
    }
}

struct FileListView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            FileListView(currentFolder: Mock.files.root!, account: Mock.accounts.account!, moving: .constant(.none))
                .mockDI()
        }
    }
}
