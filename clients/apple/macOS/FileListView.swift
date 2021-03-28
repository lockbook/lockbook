import SwiftUI
import SwiftLockbookCore

struct FileListView: View {
    
    @State var selectedItem: FileMetadata? = nil
    
    @ObservedObject var core: GlobalState
    let currentFolder: FileMetadata
    let account: Account
    
    init(core: GlobalState, currentFolder: FileMetadata, account: Account) {
        self.core = core
        self.account = account
        self.currentFolder = currentFolder
    }
    
    var body: some View {
        VStack {
            OutlineSection(core: core, root: currentFolder, selectedItem: $selectedItem)
            VStack (spacing: 3) {
                BottomBar(core: core)
            }
        }
        if let item = selectedItem {
            EditorLoader(content: core.openDocument, meta: item, files: core.files, deleteChannel: core.deleteChannel)
        }
    }
}
