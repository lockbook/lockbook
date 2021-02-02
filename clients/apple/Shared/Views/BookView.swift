import SwiftUI
import SwiftLockbookCore

struct BookView: View {
    
    @ObservedObject var core: GlobalState
    let currentFolder: FileMetadata
    let account: Account
    
    var body: some View {
        NavigationView {
            FileListView(core: core, currentFolder: currentFolder, account: account)

        }
    }
}

struct BookView_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            BookView(core: GlobalState(), currentFolder: FakeApi.root, account: .fake(username: "jeff"))
                .ignoresSafeArea()
        }
    }
}
