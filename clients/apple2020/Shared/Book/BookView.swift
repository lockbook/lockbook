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

struct FileBrowserView_Previews: PreviewProvider {
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

    var body: some View {
        let baseView = List {
            OutlineGroup(core.grouped, children: \.children) { meta in
                if meta.meta.fileType == .Folder {
                    FileCell(meta: meta.meta)
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
                .navigationBarItems(leading: NavigationLink(destination: AccountView(core: core, account: account)) {
                    Image(systemName: "person.circle.fill")
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
                    }
                }
        #endif
    }
}
