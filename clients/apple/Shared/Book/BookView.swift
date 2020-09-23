import SwiftUI
import SwiftLockbookCore

struct BookView: View {
    @ObservedObject var core: Core
    let account: Account
    let root: FileMetadata
    
    var body: some View {
        NavigationView {
            FileListView(core: core, account: account, selectedFolder: FileMetadataWithChildren(meta: root, children: []))
            
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
    @State var selectedFolder: FileMetadataWithChildren
    @State var showingCreate: Bool = false

    var body: some View {
        let baseView = List {
            OutlineGroup(core.grouped, children: \.children) { meta in
                if meta.meta.fileType == .Folder {
                    FileCell(meta: meta.meta)
                        .foregroundColor(meta.id == selectedFolder.id ? .accentColor : .primary)
                        .onTapGesture {
                            selectedFolder = meta
                        }
                } else {
                    NavigationLink(destination: EditorView(core: core, meta: meta.meta)) {
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
                    .sheet(isPresented: $showingCreate, content: {
                        CreateFileView(core: core, isPresented: $showingCreate, currentFolder: selectedFolder)
                    })
                }, trailing: Button(action: self.core.sync) {
                    Image(systemName: "arrow.2.circlepath.circle.fill")
                })
        #else
            return baseView
                .toolbar {
                    HStack {
                        Button(action: self.core.sync) {
                            Image(systemName: "arrow.2.circlepath.circle.fill")
                        }
                        Button(action: { showingCreate.toggle() }) {
                            Image(systemName: "plus.circle")
                        }
                        .keyboardShortcut(KeyEquivalent("j"), modifiers: .command)
                    }
                }
                .sheet(isPresented: $showingCreate, content: {
                    CreateFileView(core: core, isPresented: $showingCreate, currentFolder: selectedFolder)
                        .padding(100)
                })
        #endif
    }
}

struct FlipToggleStyle: ToggleStyle {
    typealias Side = (String, systemImage: String, color: Color)
    let left: Side
    let right: Side
    
    func makeBody(configuration: Configuration) -> some View {
        Button(action: {
            configuration.isOn.toggle()
        }) {
            HStack {
                Label(left.0, systemImage: left.systemImage)
                    .foregroundColor(left.color)
                    .opacity(configuration.isOn ? 1 : 0.3)
                Text("/")
                    .foregroundColor(.black)
                Label(right.0, systemImage: right.systemImage)
                    .foregroundColor(right.color)
                    .opacity(configuration.isOn ? 0.3 : 1)
                
            }
        }
    }
}

struct BookView_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            BookView(core: Core(), account: Account(username: "test"), root: FakeApi().root)
                .ignoresSafeArea()
            BookView(core: Core(), account: Account(username: "test"), root: FakeApi().root)
                .ignoresSafeArea()
                .preferredColorScheme(.dark)
            Toggle("Folder", isOn: .constant(true))
                .toggleStyle(FlipToggleStyle(left: ("Doc", "doc", .pink), right: ("Folder", "folder", .purple)))
                .padding()
                .previewLayout(.sizeThatFits)
        }
    }
}
