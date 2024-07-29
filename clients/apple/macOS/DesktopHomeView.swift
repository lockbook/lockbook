import SwiftUI
import SwiftLockbookCore
import DSFQuickActionBar
import SwiftWorkspace

struct DesktopHomeView: View {
    var body: some View {
        NavigationView {
            SidebarView()
            
            DetailView()
        }
    }
}

struct SearchBar: View {
    @Binding var searchInput: String
    @FocusState var isFocused: Bool
    
    @EnvironmentObject var files: FileService
    
    var body: some View {
        HStack {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(.gray)
            
            TextField("Search", text: $searchInput)
                .focused($isFocused)
                .onAppear {
                    isFocused = false
                }
                .onExitCommand {
                    searchInput = ""
                    isFocused = false
                }
                .textFieldStyle(.plain)
                .background(
                    Button("Search Paths And Content") {
                        isFocused = true
                    }
                    .keyboardShortcut("F", modifiers: [.command, .shift])
                    .hidden()
                )
                .onChange(of: isFocused, perform: { newValue in
                    if isFocused {
                        DI.search.startSearchThread(isPathAndContentSearch: true)
                    } else if !isFocused && searchInput.isEmpty {
                        searchInput = ""
                        DI.search.endSearch(isPathAndContentSearch: true)
                    }
                })
            
            if isFocused {
                Button(action: {
                    searchInput = ""
                    isFocused = false
                }, label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.gray)
                })
            }
        }
        .padding(5)
        .modifier(SearchBarSelectionBackgroundModifier(isFocused: $isFocused))
        .padding(.horizontal, 10)
        .padding(.bottom, 10)
        .opacity(files.root != nil ? 1 : 0)
    }
}

struct SearchBarSelectionBackgroundModifier: ViewModifier {
    @FocusState<Bool>.Binding var isFocused: Bool
    
    func body(content: Content) -> some View {
        let rectangle = RoundedRectangle(cornerRadius: 5)
        
        return content
            .background(
                rectangle
                    .fill(Color.gray)
                    .opacity(0.2)
                    .overlay(
                        isFocused ? rectangle.stroke(Color(nsColor: .selectedContentBackgroundColor).opacity(0.5), lineWidth: 3) : nil
                    )
            )
    }
}

struct SidebarView: View {
    @EnvironmentObject var search: SearchService
    
    @State var searchInput: String = ""
    @State var treeBranchState: Bool = true
            
    var body: some View {
        VStack {
            SearchBar(searchInput: $searchInput)
            
            if search.isPathAndContentSearching {
                if !search.isPathAndContentSearchInProgress && !search.pathAndContentSearchQuery.isEmpty && search.pathAndContentSearchResults.isEmpty {
                    noSearchResultsView
                } else {
                    ScrollView {
                        if search.isPathAndContentSearchInProgress {
                            ProgressView()
                                .controlSize(.small)
                                .padding(.horizontal)
                        }
                        
                        if !search.pathAndContentSearchResults.isEmpty {
                            searchResultsView
                        }
                    }
                }
            } else {
                suggestedAndFilesView
            }
        }
        .onChange(of: searchInput) { newInput in
            DI.search.search(query: newInput, isPathAndContentSearch: true)
        }
    }
    
    var noSearchResultsView: some View {
        VStack {
            Text("No results.")
                .font(.headline)
                .foregroundColor(.gray)
                .fontWeight(.bold)
                .padding()
            
            Spacer()
        }
    }
    
    var searchResultsView: some View {
        ForEach(search.pathAndContentSearchResults) { result in
            switch result {
            case .PathMatch(_, let meta, let name, let path, let matchedIndices, _):
                Button(action: {
                    DI.workspace.requestOpenDoc(meta.id)
                }) {
                    SearchFilePathCell(name: name, path: path, matchedIndices: matchedIndices)
                }
                .padding(.horizontal)

            case .ContentMatch(_, let meta, let name, let path, let paragraph, let matchedIndices, _):
                Button(action: {
                    DI.workspace.requestOpenDoc(meta.id)
                }) {
                    SearchFileContentCell(name: name, path: path, paragraph: paragraph, matchedIndices: matchedIndices)
                }
                .padding(.horizontal)
            }
            
            Divider()
                .padding(.leading, 20)
                .padding(.vertical, 5)
        }
    }

    
    var suggestedAndFilesView: some View {
        VStack {
            SuggestedDocs()

            fileTreeView
                
            BottomBar()
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
                FileTreeView()
                    .equatable()
                    .padding(.leading, 4)
                Spacer()
            } else {
                Spacer()
            }
        }
    }
}

struct DetailView: View {
    @EnvironmentObject var search: SearchService
    @EnvironmentObject var share: ShareService
    @EnvironmentObject var workspace: WorkspaceState
        
    var body: some View {
        ZStack {
            WorkspaceView(DI.workspace, DI.coreService.corePtr)
                .equatable()
                .opacity(workspace.pendingSharesOpen ? 0.0 : 1.0)
            
            if workspace.pendingSharesOpen {
                PendingSharesView()
            }
        }
        .toolbar {
            ToolbarItemGroup {
                if let id = workspace.openDoc,
                   let meta = DI.files.idsAndFiles[id],
                   !workspace.pendingSharesOpen {
                    ZStack {
                        Button(action: {
                            NSApp.keyWindow?.toolbar?.items.first?.view?.exportFileAndShowShareSheet(meta: meta)
                        }, label: {
                            Label("Share externally to...", systemImage: "square.and.arrow.up.fill")
                                .imageScale(.large)
                        })
                        .foregroundColor(.blue)
                        .padding(.trailing, 10)
                    }
                    
                    Button(action: {
                        DI.sheets.sharingFileInfo = meta
                    }, label: {
                        Label("Share", systemImage: "person.wave.2.fill")
                            .imageScale(.large)
                    })
                    .foregroundColor(.blue)
                    .padding(.trailing, 5)
                }
                
                Button(action: {
                    DI.workspace.pendingSharesOpen.toggle()
                }) {
                    pendingShareToolbarIcon(isPendingSharesEmpty: share.pendingShares?.isEmpty ?? true)
                        .imageScale(.large)
                }
            }
        }
    }
}
