import SwiftUI
import SwiftLockbookCore

struct BookView: View {
    @ObservedObject var core: Core
    let account: Account
    
    var body: some View {
        #if os(iOS)
        let placement = ToolbarItemPlacement.navigationBarTrailing
        #else
        let placement = ToolbarItemPlacement.status
        #endif

        NavigationView {
            makeList()
                .navigationTitle(account.username)
                .toolbar {
                    ToolbarItem(placement: placement) {
                        Button(action: core.sync) {
                            SyncIndicator(syncing: $core.syncing)
                                .foregroundColor(core.syncing ? .pink : .accentColor)
                        }
                        .disabled(core.syncing)
                    }
                }

            Text("Pick a file!")
        }
    }
    
    func getFiles() -> [FileMetadata] {
        switch core.api.listFiles() {
        case .success(let files):
            return files
        case .failure(let err):
            core.handleError(err)
            return []
        }
    }

    func makeList() -> some View {
        switch core.root {
        case .some(let root):
            return AnyView(FileListView(core: core, account: account, root: root))
        case .none:
            return AnyView(VStack {
                Text("Please sync!")
            })
        }
    }
}

struct BookView_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            BookView(core: Core(), account: .fake(username: "jeff"))
                .ignoresSafeArea()
//            BookView(core: Core(), account: .fake(username: "test"))
//                .ignoresSafeArea()
//                .preferredColorScheme(.dark)
        }
    }
}
