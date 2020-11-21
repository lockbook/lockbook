import SwiftUI
import SwiftLockbookCore

struct BookView: View {
    @ObservedObject var core: Core
    let account: Account
    
    var body: some View {
        NavigationView {
            #if os(iOS)
            FileListView(core: core, account: account, selectedFile: core.grouped.first)
                .navigationBarTitleDisplayMode(.inline)
            #else
            FileListView(core: core, account: account, selectedFile: core.grouped.first)
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
}

struct BookView_Previews: PreviewProvider {
    static var previews: some View {
        Group {
            BookView(core: Core(), account: .fake(username: "test"))
                .ignoresSafeArea()
//            BookView(core: Core(), account: .fake(username: "test"))
//                .ignoresSafeArea()
//                .preferredColorScheme(.dark)
        }
    }
}
