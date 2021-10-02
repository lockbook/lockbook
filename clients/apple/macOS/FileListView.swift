import SwiftUI
import SwiftLockbookCore

struct FileListView: View {
        
    @EnvironmentObject var coreService: CoreService
    @EnvironmentObject var files: FileService
    
    @StateObject var outlineState = OutlineState()
    
    let currentFolder: ClientFileMetadata
    let account: Account
    
    init(currentFolder: ClientFileMetadata, account: Account) {
        self.account = account
        self.currentFolder = currentFolder
    }
    
    var body: some View {
        VStack {
            OutlineSection(state: outlineState, root: currentFolder)
            VStack (spacing: 3) {
                BottomBar()
            }
        }
        if let item = outlineState.selectedItem {
            DocumentView(meta: item)
        }
    }
}
