import SwiftUI

struct SearchContainerView<Content: View>: View {
    @StateObject var model: SearchContainerViewModel
    @ViewBuilder let content: Content
    
    init(filesModel: FilesViewModel, content: @escaping () -> Content) {
        self._model = StateObject(wrappedValue: SearchContainerViewModel(filesModel: filesModel))
        self.content = content()
    }
    
    var body: some View {
        SearchContainerWrapperView(model: model) {
            content
        }
        .searchable(text: $model.input, placement: .navigationBarDrawer(displayMode: .automatic), prompt: "Search")
    }
}

struct SearchContainerWrapperView<Content: View>: View {
    @Environment(\.isSearching) var isSearching
    @Environment(\.dismissSearch) private var dismissSearch
    
    @State var passedIsSearching: Bool = false
    
    @ObservedObject var model: SearchContainerViewModel
    @ViewBuilder let content: Content
    
    var body: some View {
        SearchContainerSubView(isSearching: $passedIsSearching, model: model, dismissSearch: { dismissSearch() }, content: content)
            .onChange(of: model.input) { _ in
                model.search()
            }
            .onChange(of: isSearching) { newValue in
                passedIsSearching = newValue
            }
    }
}
