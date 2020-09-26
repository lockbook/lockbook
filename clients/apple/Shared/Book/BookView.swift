import SwiftUI
import SwiftLockbookCore

struct BookView: View {
    @ObservedObject var core: Core
    let account: Account
    
    var body: some View {
        NavigationView {
            FileListView(core: core, account: account)
            
            Text("Pick a file!")
        }
    }
    
    func getFiles() -> [FileMetadata] {
        switch core.api.listFiles() {
        case .success(let files):
            return files
        case .failure(let err):
            core.displayError(error: err)
            return []
        }
    }
}

struct FileCell: View {
    let meta: FileMetadata
    
    var body: some View {
        VStack(alignment: .leading) {
            Text(meta.name)
            Label(intEpochToString(epoch: meta.contentVersion), systemImage: meta.fileType == .Folder ? "folder" : "doc")
                .font(.footnote)
                .foregroundColor(.secondary)
        }
    }
}

struct FileListView: View {
    @ObservedObject var core: Core
    let account: Account
    @State var selectedFolder: FileMetadataWithChildren?
    @State var showingCreate: Bool = false

    var body: some View {
        let baseView = List {
            OutlineGroup(core.grouped, children: \.children) { meta in
                if meta.meta.fileType == .Folder {
                    FileCell(meta: meta.meta)
                        .foregroundColor(meta.id == selectedFolder?.id ? .accentColor : .primary)
                        .onLongPressGesture {
                            selectedFolder = meta
                            showingCreate = true
                        }
                        .onTapGesture {
                            selectedFolder = meta
                        }
                } else {
                    NavigationLink(destination: EditorView(core: core, meta: meta.meta).equatable()) {
                        FileCell(meta: meta.meta)
                    }
                }
            }
            HStack {
                Spacer()
                Text("\(core.files.count) items")
                    .foregroundColor(.secondary)
                Spacer()
            }
        }
        .listStyle(InsetListStyle())
        .navigationTitle("\(account.username)'s files")
        .onReceive(core.timer, perform: { _ in
            core.sync()
        })
        
        #if os(iOS)
            return baseView
                .navigationBarItems(leading: HStack {
                    NavigationLink(destination: AccountView(core: core, account: account)) {
                        Image(systemName: "person.circle.fill")
                    }
                    Button(action: { showingCreate.toggle() }) {
                        Image(systemName: "plus.circle")
                    }
                    .keyboardShortcut(KeyEquivalent("j"), modifiers: .command)
                    .popover(isPresented: $showingCreate, content: {
                        if let folder = selectedFolder {
                            CreateFileView(core: core, isPresented: $showingCreate, currentFolder: folder)
                                .padding(50)
                        } else {
                            Text("Select a folder first!")
                                .padding()
                        }
                    })
                }, trailing: HStack {
                    Button(action: core.sync) {
                        SyncIndicator(syncing: $core.syncing)
                            .foregroundColor(core.syncing ? .pink : .accentColor)
                    }
                    .disabled(core.syncing)
                })
        #else
            return baseView
                .toolbar {
                    HStack {
                        Button(action: core.sync) {
                            SyncIndicator(syncing: $core.syncing)
                                .foregroundColor(core.syncing ? .pink : .accentColor)
                        }
                        .disabled(core.syncing)
                        Button(action: { showingCreate.toggle() }) {
                            Image(systemName: "plus.circle")
                        }
                        .keyboardShortcut(KeyEquivalent("j"), modifiers: .command)
                        .popover(isPresented: $showingCreate, content: {
                            if let folder = selectedFolder {
                                CreateFileView(core: core, isPresented: $showingCreate, currentFolder: folder)
                                    .padding(50)
                            } else {
                                Text("Select a folder first!")
                                    .padding()
                            }
                        })
                    }
                }
        #endif
    }
}

struct BookView_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            BookView(core: Core(), account: Account(username: "test"))
                .ignoresSafeArea()
            BookView(core: Core(), account: Account(username: "test"))
                .ignoresSafeArea()
                .preferredColorScheme(.dark)
        }
    }
}
