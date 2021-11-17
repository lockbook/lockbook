import SwiftUI
import SwiftLockbookCore

struct BookView: View {
    
    let currentFolder: DecryptedFileMetadata
    let account: Account
    
    @State var moving: DecryptedFileMetadata?

    var body: some View {
        NavigationView {
            #if os(iOS)
            FileListView(currentFolder: currentFolder, account: account, moving: $moving)
            #else
            FileListView(currentFolder: currentFolder, account: account)
            #endif
        }
    }
}

struct BookView_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            BookView(currentFolder: FakeApi.root, account: .fake(username: "jeff"))
                .ignoresSafeArea()
        }
    }
}
