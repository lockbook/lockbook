import SwiftUI
import SwiftLockbookCore
import DSFQuickActionBar

struct FileListView: View {
    @State var searchInput: String = ""
    @State var expandedFolders: [File] = []
    @State var lastOpenDoc: File? = nil
    
    @State var treeBranchState: Bool = true
        
    var body: some View {
        VStack {
            SearchWrapperView(
                searchInput: $searchInput,
                mainView: mainView,
                isiOS: false)
            .searchable(text: $searchInput, prompt: "Search")
                
            BottomBar()
        }
            
        DetailView()
    }
    
    var mainView: some View {
        VStack {
            SuggestedDocs()

            fileTreeView
        }
    }
    
    var fileTreeView: some View {
        Group {
            Button(action: {
                withAnimation {
                    treeBranchState.toggle()
                }
            }) {
                HStack {
                    Text("Files")
                        .bold()
                        .foregroundColor(.gray)
                        .font(.subheadline)
                    Spacer()
                    if treeBranchState {
                        Image(systemName: "chevron.down")
                            .foregroundColor(.gray)
                            .imageScale(.small)
                    } else {
                        Image(systemName: "chevron.right")
                            .foregroundColor(.gray)
                            .imageScale(.small)
                    }
                }
                .padding(.top)
                .padding(.horizontal)
                .contentShape(Rectangle())
            }
            
            if treeBranchState {
                FileTreeView(expandedFolders: $expandedFolders, lastOpenDoc: $lastOpenDoc)
                    .padding(.leading, 4)
                Spacer()
            } else {
                Spacer()
            }
        }
    }
}

struct DetailView: View {
    @EnvironmentObject var currentSelection: DocumentService
    @EnvironmentObject var search: SearchService
    @EnvironmentObject var share: ShareService
    
    @State var quickActionBarVisible = false
    @State var selectedFile: SearchResultItem? = nil
    
    var body: some View {
        ZStack {
            if currentSelection.isPendingSharesOpen {
                PendingSharesView()
            } else {
                DocumentTabView()
            }
            
            QuickActionBar<SearchResultItem, SearchResultCellView>(
                location: .window,
                visible: $search.isPathSearching,
                barWidth: 400,
                showKeyboardShortcuts: true,
                selectedItem: $selectedFile,
                placeholderText: "Search files",
                itemsForSearchTerm: { searchTask in
                    let maybeSearchResults = search.searchFilePath(input: searchTask.searchTerm)
                    
                    if let results = maybeSearchResults {
                        searchTask.complete(with: results)
                    }
                },
                viewForItem: { searchResult, searchTerm in
                    let (name, path) = searchResult.getNameAndPath()

                    return SearchResultCellView(name: name, path: path, matchedIndices: searchResult.matchedIndices)
                }
            )
            .onChange(of: selectedFile) { newValue in
                if let submittedId = newValue?.id {
                    search.submitSearch(id: submittedId)
                }
            }
        }
        .toolbar {
            ToolbarItemGroup {
                Button(action: {
                    currentSelection.isPendingSharesOpen = true
                }) {
                    pendingShareToolbarIcon(isiOS: false, isPendingSharesEmpty: share.pendingShares.isEmpty)
                    
                }
            }
        }
    }
}

struct SearchResultCellView: View {
    let name: String
    let path: String
    let matchedIndices: [Int]
    
    @State var pathModified: Text = Text("")
    @State var nameModified: Text = Text("")
    
    var body: some View {
        HStack {
            Image(systemName: "doc.text.fill")
                .resizable()
                .frame(width: 20, height: 25)
                .padding(.horizontal, 10)
                .foregroundColor(.primary)
            
            VStack(alignment: .leading) {
                HStack {
                    nameModified
                        .font(.system(size: 16))
                        .multilineTextAlignment(.leading)
                    Spacer()
                }
                HStack {
                    pathModified
                        .multilineTextAlignment(.leading)
                    Spacer()
                }
            }
        }
        .frame(height: 40)
        .padding(EdgeInsets(top: 4, leading: 0, bottom: 4, trailing: 0))
        .onAppear {
            underlineMatchedSegments()
        }
    }
    
    func underlineMatchedSegments() {
        let matchedIndicesHash = Set(matchedIndices)
        
        var pathOffset = 1;
        
        if(path.count - 1 > 0) {
            pathModified = Text("")
            
            for index in 0...path.count - 1 {
                let correctIndex = String.Index(utf16Offset: index, in: path)
                let newPart = Text(path[correctIndex...correctIndex])
                
                if(matchedIndicesHash.contains(index + 1)) {
                    pathModified = pathModified + newPart.bold()
                } else {
                    pathModified = pathModified + newPart
                }
            }
            
            pathOffset = 2
        }
                
        if(name.count - 1 > 0) {
            nameModified = Text("")
            for index in 0...name.count - 1 {
                let correctIndex = String.Index(utf16Offset: index, in: name)
                let newPart = Text(name[correctIndex...correctIndex])
                
                if(matchedIndicesHash.contains(index + path.count + pathOffset)) {
                    nameModified = nameModified + newPart.bold()
                } else {
                    nameModified = nameModified + newPart
                }
            }
        }
    }
}
