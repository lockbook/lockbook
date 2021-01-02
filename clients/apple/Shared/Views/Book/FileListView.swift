import SwiftUI
import SwiftLockbookCore

struct FileListView: View {
    @ObservedObject var core: Core
    @State var showingAccount: Bool = false
    let currentFolder: FileMetadata
    let account: Account
    
    var children: [FileMetadata] {
        core.files.filter {
            $0.parent == currentFolder.id && $0.id != currentFolder.id
        }
    }
    
    var body: some View {
        ScrollView {
            HStack {
                Spacer()
            }
            VStack {
                ForEach(children) { meta in
                    if meta.fileType == .Folder {
                        NavigationLink(destination: FileListView(core: core, currentFolder: meta, account: account)) {
                            FileCell(meta: meta)
                        }.isDetailLink(false)
                    } else {
                        NavigationLink(destination: EditorView(core: core, meta: meta).equatable()) {
                            FileCell(meta: meta)
                        }
                    }
                }
            }
            .cornerRadius(10)
        }
        .padding(.leading, 10)
        .sheet(isPresented: $showingAccount, content: {
            AccountView(core: core, account: account)
        })
        .toolbar {
            ToolbarItem(placement: .navigationBarLeading) {
                Button(action: { showingAccount.toggle() }) {
                    Image(systemName: "person.circle.fill")
                }
            }
            ToolbarItem(placement: .navigationBarTrailing) {
                Button(action: core.sync) {
                    Image(systemName: "arrow.right.arrow.left.circle.fill")
                }
            }
            ToolbarItemGroup(placement: .bottomBar) {
                ProgressView()
                    .opacity(core.syncing ? 1.0 : 0)
                Spacer()
                Text("\(core.files.count) items")
                    .foregroundColor(.secondary)
                Spacer()
                Button(action: { }) {
                    Image(systemName: "folder.badge.plus")
                }
                Button(action: { }) {
                    Image(systemName: "square.and.pencil")
                }
            }
        }
        .navigationTitle(currentFolder.name)
    }
}

