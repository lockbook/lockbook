import SwiftUI
import SwiftLockbookCore

struct FileListView: View {
    
    @EnvironmentObject var search: SearchService

    var body: some View {
        ZStack {
            VStack {
                FileTreeView()
                VStack (spacing: 3) {
                    BottomBar()
                }
            }
            
            DetailView()
        }
        
        if search.isSearching {
            SearchPathsView()
        }
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
