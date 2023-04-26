import SwiftUI
import SwiftLockbookCore
import DSFQuickActionBar

struct FileListView: View {
    
    @State var searchInput: String = ""
    @State var expandedFolders: [File] = []
    @State var lastOpenDoc: File? = nil
    
    @StateObject var treeBranchState: BranchState = BranchState(open: true)
        
    var body: some View {
        VStack {
            SearchWrapperView(
                searchInput: $searchInput,
                mainView: mainView,
                isiOS: false)
            .searchable(text: $searchInput, prompt: "Search")
            .keyboardShortcut(.escape)
                
            VStack (spacing: 3) {
                BottomBar()
            }
        }
            
        DetailView()
    }
    
    var mainView: some View {
        VStack {
            SuggestedDocumentsView()
            fileTreeView
        }
    }
    
    var fileTreeView: some View {
        Group {
            Button(action: {
                withAnimation {
                    treeBranchState.open.toggle()
                }
            }) {
                HStack {
                    Text("Tree")
                        .bold()
                        .foregroundColor(.gray)
                        .font(.subheadline)
                    Spacer()
                    if treeBranchState.open {
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
            
            if treeBranchState.open {
                FileTreeView(expandedFolders: $expandedFolders, lastOpenDoc: $lastOpenDoc)
                    .padding(.leading, 4)
            } else {
                Spacer()
            }
        }
    }
}

struct SuggestedDocumentsView: View {
    @StateObject var suggestedDocsBranchState: BranchState = BranchState(open: false)
    
    @EnvironmentObject var current: CurrentDocument
    @EnvironmentObject var fileService: FileService
    
    var body: some View {
        Group {
            Button(action: {
                withAnimation {
                    suggestedDocsBranchState.open.toggle()
                }
            }) {
                HStack {
                    Text("Suggested")
                        .bold()
                        .foregroundColor(.gray)
                        .font(.subheadline)
                    Spacer()
                    if suggestedDocsBranchState.open {
                        Image(systemName: "chevron.down")
                            .foregroundColor(.gray)
                            .imageScale(.small)
                    } else {
                        Image(systemName: "chevron.right")
                            .foregroundColor(.gray)
                            .imageScale(.small)
                    }
                }
                .padding(.horizontal)
                .contentShape(Rectangle())
            }
            
            if suggestedDocsBranchState.open {
                if !fileService.suggestedDocs.isEmpty {
                    ForEach(fileService.suggestedDocs, id: \.id) { meta in
                        Button(action: {
                            current.selectedDocument = meta
                        }) {
                            macOSSuggestedDocCell(name: meta.name, duration: meta.lastModified)
                        }
                    }
                }
            }
        }

    }
}

struct DetailView: View {
    @EnvironmentObject var currentSelection: CurrentDocument
    @EnvironmentObject var search: SearchService
    
    @State var quickActionBarVisible = false
    @State var selectedFile: SearchResultItem? = nil
    
    var body: some View {
        ZStack {
            VStack {
                if let selected = currentSelection.selectedDocument {
                    DocumentView(meta: selected)
                }
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

struct macOSSuggestedDocCell: View {
    let name: String
    let duration: UInt64
    
    var body: some View {
        HStack {
            Image(systemName: "doc.circle")
                .resizable()
                .scaledToFill()
                .frame(width: 21, height: 21)
                .foregroundColor(.accentColor)
            
            VStack(alignment: .leading) {
                Text(name)
                    .font(.callout)
                
                Text(timeAgo(epoch: duration))
                    .foregroundColor(.gray)
                    .font(.callout)
            }
            .padding(.leading, 5)
            
            Spacer()
        }
        .padding(.horizontal)
        .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
    }
}

