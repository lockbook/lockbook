import SwiftUI
import SwiftLockbookCore

struct FileListView: View {
    
    @State var selectedItem: ClientFileMetadata? = nil
    
    @EnvironmentObject var files: FileService
    
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
                ImageLoader(meta: item)
            } else {
                EditorLoader(meta: item)
            }
        }
    }
}
