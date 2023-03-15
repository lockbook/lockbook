import SwiftUI
import SwiftLockbookCore
import DSFQuickActionBar

struct FileListView: View {
    

    var body: some View {
        VStack {
            FileTreeView()
            VStack (spacing: 3) {
                BottomBar()
            }
        }
        
        VStack {
            DetailView()
        }
    }
}

struct DetailView: View {
    @EnvironmentObject var currentSelection: CurrentDocument
    
    @EnvironmentObject var search: SearchService

    
    @State var quickActionBarVisible = false
    @State var selectedFile: SearchResultItem? = nil


    var body: some View {
        VStack {
            if let selected = currentSelection.selectedDocument {
                DocumentView(meta: selected)
            }
            
            QuickActionBar<SearchResultItem, SearchResultCellView>(
                location: .window,
                visible: $search.isSearching,
                barWidth: 350,
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
            .frame(minWidth: 0, maxWidth: 0,    minHeight: 0, maxHeight: 0)
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
                .foregroundColor(.purple)
            
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
        .frame(height: 45)
        .padding(EdgeInsets(top: 4, leading: 0, bottom: 4, trailing: 0))
        .onAppear {
            underlineMatchedSegments()
        }
    }
    
    func underlineMatchedSegments() {
        let matchedIndicesHash = Set(matchedIndices)
        
        pathModified = Text("")
        
        print("Checking the path \(path)")
        
        for index in 0...path.count - 1 {
            print("CHECKING PATH INDEX \(index)")
            
            var newPart = Text(path[String.Index(encodedOffset: index)...String.Index(encodedOffset: index)])
            
            if(matchedIndicesHash.contains(index + 1)) {
                pathModified = pathModified + newPart.foregroundColor(.black)
            } else {
                pathModified = pathModified + newPart.foregroundColor(.gray)
            }
        }
        
        print("Checking the name \(name)")
        
        nameModified = Text("")
        for index in 0...name.count - 1 {
            print("CHECKING PATH INDEX \(index)")

            var newPart = Text(name[String.Index(encodedOffset: index)...String.Index(encodedOffset: index)])
            
            if(matchedIndicesHash.contains(index + path.count + 2)) {
                nameModified = nameModified + newPart.foregroundColor(.black)
            } else {
                nameModified = nameModified + newPart.foregroundColor(.gray)
            }
        }
    }
    
    
}

