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
                if let id = currentSelection.selectedDoc,
                   let meta = DI.files.idsAndFiles[id],
                   !currentSelection.isPendingSharesOpen {
                    
                    let view = MacOSShareSpaceHolder()
                    
                    ZStack {
                        view.id(UUID())
                        
                        Button(action: {
                            view.view.exportFileAndShowShareSheet(meta: meta)
                        }, label: {
                            Label("Share externally to...", systemImage: "person.wave.2.fill")
                                .imageScale(.large)
                        })
                        .foregroundColor(.blue)
                        .padding(.trailing, 10)
                    }
                    
                    Button(action: {
                        DI.sheets.sharingFileInfo = meta
                    }, label: {
                        Label("Share", systemImage: "square.and.arrow.up.fill")
                            .imageScale(.large)
                    })
                    .foregroundColor(.blue)
                    .padding(.trailing, 5)
                }
                
                Button(action: {
                    currentSelection.isPendingSharesOpen = true
                }) {
                    pendingShareToolbarIcon(isPendingSharesEmpty: share.pendingShares.isEmpty)
                        .imageScale(.large)
                }
            }
        }
    }
}

struct MacOSShareSpaceHolder: NSViewRepresentable {
    let view = NSView()
        
    func makeNSView(context: Context) -> NSView {
        view
    }

    func updateNSView(_ nsView: NSView, context: Context) {}
}
