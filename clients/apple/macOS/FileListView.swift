import SwiftUI
import SwiftLockbookCore

struct FileListView: View {
    
    @State var currentSelection: DecryptedFileMetadata? = nil
    
    var body: some View {
        VStack {
            FileTreeView(currentSelection: $currentSelection)
            VStack (spacing: 3) {
                BottomBar()
            }
        }
        
        if let selected = currentSelection {
            DocumentView(meta: selected)
        }
    }
}
