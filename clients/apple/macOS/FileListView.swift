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
        if selectedItem != nil {
            EditorLoader(content: core.openDocument, meta: selectedItem!, files: core.files)
        }
    }
}
