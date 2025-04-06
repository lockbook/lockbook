import SwiftUI
import SwiftWorkspace


struct SearchContainerView<Content: View>: View {
    @StateObject var model = SearchContainerViewModel()
    @ViewBuilder let content: Content
    
    var body: some View {
        SearchContainerSubView(model: model, content: content)
            .modifier(SearchableMarker(model: model))
            .onChange(of: model.input) { _ in
                model.search()
            }
    }
}

struct SearchContainerSubView<Content: View>: View {
    @Environment(\.isSearching) var isSearching
    @Environment(\.dismissSearch) private var dismissSearch
    
    @EnvironmentObject var homeState: HomeState
    
    @ObservedObject var model: SearchContainerViewModel
    let content: Content
    
    var body: some View {
        Group {
            if isSearching {
                if !model.isSearchInProgress && !model.input.isEmpty && model.results.isEmpty {
                    noResults
                } else {
                    ScrollView {
                        if model.isSearchInProgress {
                            ProgressView()
                                .frame(width: 20, height: 20)
                                .padding(.top)
                        }
                        
                        if !model.results.isEmpty {
                            results
                        }
                    }
                }
            } else {
                content
            }
        }
        .onChange(of: isSearching) { newValue in
            if newValue {
                model.search()
            }
        }
    }
    
    var results: some View {
        ForEach(model.results, id: \.id) { result in
            switch result {
            case .path(let pathResult):
                Button(action: {
                    model.open(id: pathResult.id)
                    homeState.constrainedSidebarState = .closed
                    dismissSearch()
                }) {
                    SearchPathResultView(name: pathResult.path.nameAndPath().0, path: pathResult.path.nameAndPath().1, matchedIndices: pathResult.matchedIndicies)
                }
                .padding(.horizontal)
            case .document(let docResult):
                Button(action: {
                    model.open(id: docResult.id)
                    homeState.constrainedSidebarState = .closed
                    dismissSearch()
                }) {
                    SearchContentResultView(name: docResult.path.nameAndPath().0, path: docResult.path.nameAndPath().1, contentMatches: docResult.contentMatches)
                }
                .padding(.horizontal)
            }
            
            Divider()
                .padding(.leading, 20)
                .padding(.vertical, 5)
        }
    }
    
    var noResults: some View {
        VStack {
            Text("No results.")
                .font(.headline)
                .foregroundColor(.gray)
                .fontWeight(.bold)
                .padding()
            
            Spacer()
        }
    }
}

class SearchContainerViewModel: ObservableObject {
    @Published var input: String = ""
    @Published var isShown: Bool = false
    @Published var isSearchInProgress: Bool = false
    
    @Published var results: [SearchResult] = []
    
    func open(id: UUID) {
        AppState.workspaceState.requestOpenDoc(id)
    }
    
    func search() {
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.search(input: self.input, searchPaths: true, searchDocs: true)
            
            DispatchQueue.main.async {
                switch res {
                case .success(let results):
                    self.results = results
                case .failure(let err):
                    print("got error: \(err.msg)")
                }
            }
        }
    }
}

struct SearchableMarker: ViewModifier {
    @ObservedObject var model = SearchContainerViewModel()
    
    #if(iOS)
    let placement: SearchFieldPlacement =  .navigationBarDrawer(displayMode: .automatic)
    #else
    let placement: SearchFieldPlacement =  .sidebar
    #endif
    
    func body(content: Content) -> some View {
        #if(iOS)
        content.searchable(text: $model.input, placement: placement, prompt: "Search")
        #else
        content.searchable(text: $model.input, placement: placement, prompt: "Search")
        #endif
    }
}

