import SwiftUI
import SwiftLockbookCore

struct FileListView: View {
    
    @State var selectedItem: ClientFileMetadata? = nil
    
    @EnvironmentObject var coreService: CoreService
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
            let _ = print(item.name)
            DocumentView(meta: item)
        }
    }
}
