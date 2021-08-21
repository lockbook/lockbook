import SwiftUI
import SwiftLockbookCore

struct FileListView: View {
    
    @State var selectedItem: ClientFileMetadata? = nil
    
    @EnvironmentObject var core: GlobalState
    let currentFolder: ClientFileMetadata
    let account: Account
    
    init(currentFolder: ClientFileMetadata, account: Account) {
        self.account = account
        self.currentFolder = currentFolder
    }
    
    var body: some View {
        VStack {
            OutlineSection(root: currentFolder, selectedItem: $selectedItem)
            VStack (spacing: 3) {
                BottomBar()
            }
        }
        if let item = selectedItem {
            if (item.name.hasSuffix(".draw")) {
                ImageLoader(model: core.openImage, meta: item, deleteChannel: core.deleteChannel)
            } else {
                EditorLoader(content: core.openDocument, meta: item, deleteChannel: core.deleteChannel)
            }
        }
    }
}
