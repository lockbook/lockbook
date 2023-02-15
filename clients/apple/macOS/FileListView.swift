import SwiftUI
import SwiftLockbookCore

struct FileListView: View {

    var body: some View {
        VStack {
            FileTreeView()
            VStack (spacing: 3) {
                BottomBar()
            }
        }
        
        DetailView()
    }
}

struct DetailView: View {
    @EnvironmentObject var currentSelection: CurrentDocument

    var body: some View {
        if let selected = currentSelection.selectedDocument {
            DocumentView(meta: selected)
        }
    }
}
