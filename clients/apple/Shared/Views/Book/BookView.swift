import SwiftUI
import SwiftLockbookCore

struct BookView: View {
    @ObservedObject var core: Core
    @State var showingAccount: Bool = false
    let account: Account
    
    var body: some View {
        NavigationView {
            #if os(iOS)
            makeList()
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
                }
                .navigationTitle(account.username)
            #else
            makeList()
            #endif

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
            return AnyView(
                VStack(spacing: 10) {
                    Label("Please sync!", systemImage: "arrow.right.arrow.left.circle.fill")
                    #if os(macOS)
                    Text("Shift-Command-S (⇧⌘S)")
                    #endif
                }
            )
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
