import SwiftUI
import SwiftWorkspace

struct PathSearchContainerView<Content: View>: View {
    @StateObject var model: PathSearchViewModel
    @ViewBuilder var content: Content
    
    #if os(macOS)
    @FocusState var focused: Bool
    #endif
    
    init(model: PathSearchViewModel, content: @escaping () -> Content) {
        self._model = StateObject(wrappedValue: model)
        self.content = content()
    }
    
    init(filesModel: FilesViewModel, workspaceInput: WorkspaceInputState, content: @escaping () -> Content) {
        self._model = StateObject(wrappedValue: PathSearchViewModel(filesModel: filesModel, workspaceInput: workspaceInput))
        self.content = content()
    }
    
    let SEARCH_BAR_WIDTH: CGFloat = 500
    
    var body: some View {
        ZStack {
            content
            
            if model.isShown {
                searchWrapper
            }
        }
        .environmentObject(model)
        .background(
            Button("Toggle path search") {
                model.isShown.toggle()
            }
            .keyboardShortcut("o", modifiers: [.command])
            .hidden()
        )
        .onChange(of: model.isShown) { _ in
            model.selected = 0
        }
    }
    
    var searchWrapper: some View {
        Group {
            Rectangle()
                .foregroundColor(.gray.opacity(0.01))
                .edgesIgnoringSafeArea(.all)
                .onTapGesture {
                    model.endSearch()
                }
            
            GeometryReader { geometry in
                VStack {
                    HStack {
                        Image(systemName: "magnifyingglass")
                        
                        textField
                        
                        if model.isSearchInProgress {
                            progress
                        }
                    }
                    
                    if !model.results.isEmpty {
                        Divider()
                            .padding(.top)
                        
                        searchResults
                        
                    } else if !model.isSearchInProgress && model.results.isEmpty {
                        Text("No results.")
                           .font(.headline)
                           .foregroundColor(.gray)
                           .fontWeight(.bold)
                           .padding()
                    }
                }
                .padding()
                .background(
                    RoundedRectangle(cornerSize: CGSize(width: 20, height: 20))
                        .foregroundColor({
                            #if os(iOS)
                            Color(UIColor.secondarySystemBackground)
                            #else
                            Color(nsColor: .windowBackgroundColor)
                            #endif
                        }())
                        .shadow(radius: 10)
                )
                .frame(width: 500)
                .fixedSize(horizontal: false, vertical: true)
                .offset(x: (geometry.size.width / 2) - (SEARCH_BAR_WIDTH / 2), y: geometry.size.height / 4.5)
            }
        }
    }
    
    var searchResults: some View {
        ScrollViewReader { scrollHelper in
            ScrollView {
                ForEach(Array(model.results.enumerated()), id: \.element) {index, result in
                    Button(action: {
                        model.selected = index
                        model.openSelected()
                    }, label: {
                        PathSearchResultView(name: result.path.nameAndPath().0, path: result.path.nameAndPath().1, matchedIndices: result.matchedIndicies, index: index, isSelected: model.selected == index)
                    })
                    .buttonStyle(PlainButtonStyle())
                }
                .scrollIndicators(.visible)
                .padding(.horizontal)
            }
            .onChange(of: model.selected) { newValue in
                withAnimation {
                    if newValue < model.results.count {
                        scrollHelper.scrollTo(model.results[newValue], anchor: .center)
                    }
                }
            }
        }
        .frame(maxHeight: 500)
    }
    
    var textField: some View {
        #if os(iOS)
        PathSearchTextFieldWrapper()
            .frame(height: 30)
        #else
        PathSearchTextFieldWrapper()
            .focused($focused)
            .onAppear {
                focused = true
            }
        #endif
    }
    
    var progress: some View {
        #if os(iOS)
        ProgressView()
            .frame(width: 20, height: 20)
        #else
        ProgressView()
            .scaleEffect(0.5)
            .frame(width: 20, height: 20)
        #endif
    }
}

#Preview("Path Search") {
    var pathSearchModel = PathSearchViewModel(filesModel: FilesViewModel(), workspaceInput: WorkspaceInputState())
    pathSearchModel.isShown = true
    
    return PathSearchContainerView(model: pathSearchModel, content: {
        Color.red
    })
}

#Preview("Path Search Single Item") {
    var pathSearchModel = PathSearchViewModel(filesModel: FilesViewModel(), workspaceInput: WorkspaceInputState())
    pathSearchModel.isShown = true
    pathSearchModel.results = [
        PathSearchResult(id: UUID(), path: "/", score: 1, matchedIndicies: [])
    ]
    
    return PathSearchContainerView(model: pathSearchModel) {
        Color.red
    }

}
