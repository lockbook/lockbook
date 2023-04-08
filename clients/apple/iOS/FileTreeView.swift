import SwiftUI
import SwiftLockbookCore

struct FileTreeView: View {

    @EnvironmentObject var sheets: SheetState
    @EnvironmentObject var currentDoc: CurrentDocument
    @EnvironmentObject var coreService: CoreService
    @EnvironmentObject var files: FileService
    @EnvironmentObject var onboarding: OnboardingService
    @EnvironmentObject var search: SearchService
    
    @State var searchInput: String = ""

    let currentFolder: File
    let account: Account
    
    var body: some View {
        VStack {
            iPadFileItems(currentFolder: currentFolder)
                .searchable(text: $searchInput, placement: .navigationBarDrawer(displayMode: .automatic), prompt: "Search")
                .onChange(of: searchInput) { newInput in
                    if (!newInput.isEmpty) {
                        search.search(query: newInput)
                    } else {
                        search.searchPathAndContentState = .Idle
                    }
                }
            HStack {
                BottomBar(onCreating: {
                    sheets.creatingInfo = CreatingInfo(parent: currentFolder, child_type: .Document)
                })
            }
        }
        
        VStack {
            if let item = currentDoc.selectedDocument {
                DocumentView(meta: item)
            } else {
                GeometryReader { geometry in
                    if geometry.size.height > geometry.size.width {
                        VStack {
                            Image(systemName: "rectangle.portrait.lefthalf.inset.filled")
                                .font(.system(size: 60))
                                .padding(.bottom, 10)
                            
                            
                            Text("No document is open. Expand the file tree by swiping from the left edge of the screen or clicking the button on the top left corner.")
                                .font(.title2)
                                .multilineTextAlignment(.center)
                                .frame(maxWidth: 350)
                        }
                        .padding(.horizontal)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                    } else {
                        EmptyView()
                    }
                }
            }
        }
        .toolbar {
            ToolbarItemGroup(placement: .navigationBarTrailing) {
                NavigationLink(
                    destination: PendingSharesView()) {
                        Image(systemName: "person.3.fill")
                            .foregroundColor(.blue)
                    }
                    
                NavigationLink(
                    destination: SettingsView().equatable(), isActive: $onboarding.theyChoseToBackup) {
                        Image(systemName: "gearshape.fill")
                            .foregroundColor(.blue)
                            .padding(.horizontal, 10)
                    }
            }
        }
        
    }
}

struct iPadFileItems: View {
    @EnvironmentObject var search: SearchService
    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var current: CurrentDocument
    
    @Environment(\.isSearching) var isSearching
    
    let currentFolder: File
    
    var body: some View {
        VStack {
            switch search.searchPathAndContentState {
            case .NotSearching:
                OutlineSection(root: currentFolder)
            case .Idle:
                Spacer()
            case .NoMatch:
                Spacer()
                Text("No search results")
                Spacer()
            case .Searching:
                Spacer()
                ProgressView()
                Spacer()
            case .SearchSuccessful(let results):
                List(results) { result in
                    switch result {
                    case .PathMatch(_, let meta, let name, let path, let matchedIndices, _):
                        Button(action: {
                            current.selectedDocument = meta
                        }) {
                            SearchFilePathCell(name: name, path: path, matchedIndices: matchedIndices)
                        }
                    case .ContentMatch(_, let meta, let name, let path, let paragraph, let matchedIndices, _):
                        Button(action: {
                            current.selectedDocument = meta
                        }) {
                            SearchFileContentCell(name: name, path: path, paragraph: paragraph, matchedIndices: matchedIndices)
                        }
                    }
                }
                .listStyle(.inset)
            }
        }
        .onChange(of: isSearching, perform: { newInput in
            if newInput {
                search.startSearchThread()
            } else {
                search.endSearch()
            }
        })
    }
}

