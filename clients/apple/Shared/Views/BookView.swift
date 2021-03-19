import SwiftUI
import SwiftLockbookCore

struct BookView: View {
    
    @ObservedObject var core: GlobalState
    let currentFolder: FileMetadata
    let account: Account
    @State var moving: FileMetadata?

    var body: some View {
        NavigationView {
            #if os(iOS)
            FileListView(core: core, currentFolder: currentFolder, account: account, moving: $moving)
            #else
            FileListView(core: core, currentFolder: currentFolder, account: account)
            #endif
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
