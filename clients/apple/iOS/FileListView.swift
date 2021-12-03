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
    @EnvironmentObject var settings: SettingsService
    
    @State var creatingFile: Bool = false
    @State var creating: FileType?
    @State var creatingName: String = ""
    let currentFolder: DecryptedFileMetadata
    let account: Account
    @Binding var moving: DecryptedFileMetadata?
    @State var renaming: DecryptedFileMetadata?
    @State private var selection: DecryptedFileMetadata?
    
    var files: [DecryptedFileMetadata] {
        fileService.files.filter {
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
            .onAppear { // Different from willEnterForeground because its called on startup
                settings.calculateServerUsageDuringInitialLoad()
            }
            .onReceive(NotificationCenter.default.publisher(for: UIApplication.willResignActiveNotification)) { _ in
                sync.sync()
            }
            .onReceive(NotificationCenter.default.publisher(for: UIApplication.willEnterForegroundNotification)) { _ in
                sync.sync()
            }
            .navigationBarTitle(currentFolder.decryptedName)
            if settings.showUsageAlert {
                NavigationLink(
                    destination: SettingsView(account: account).equatable()) {
                        UsageBanner()
                    }
            }
            HStack {
                BottomBar(onCreating: { creatingFile = true })
            }
            .padding(.horizontal, 10)
            .sheet(isPresented: $onboarding.anAccountWasCreatedThisSession, content: { BeforeYouStart() })
            .sheet(isPresented: $creatingFile, content: {NewFileSheet(parent: currentFolder, onSuccess: fileSuccessfullyCreated)})
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
    
    func fileSuccessfullyCreated(new: DecryptedFileMetadata) {
        creatingFile = false
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(1000)) {
            selection = new
        }
    }
}

struct FileListView_Previews: PreviewProvider {
    static var previews: some View {
        NavigationView {
            FileListView(currentFolder: Mock.dummyRoot, account: Mock.dummyAccount, moving: .constant(.none))
                .mockDI()
        }
    }
}
