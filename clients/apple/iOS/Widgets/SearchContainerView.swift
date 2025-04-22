import SwiftUI

struct SearchContainerView<Content: View>: View {
    @Environment(\.isSearching) var isSearching
    @Environment(\.dismissSearch) private var dismissSearch
    
    @State var passedIsSearching: Bool = false
    
    @StateObject var model = SearchContainerViewModel()
    @ViewBuilder let content: Content
    
    var body: some View {
        SearchContainerSubView(isSearching: $passedIsSearching, model: model, dismissSearch: { dismissSearch() }, content: content)
            .modifier(SearchableMarker(model: model))
            .onChange(of: model.input) { _ in
                model.search()
            }
            .onChange(of: isSearching) { newValue in
                passedIsSearching = newValue
            }
    }
}
