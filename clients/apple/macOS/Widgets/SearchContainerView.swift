import SwiftUI

struct SearchContainerView<Content: View>: View {
    @Environment(\.dismissSearch) private var dismissSearch
    @StateObject var model: SearchContainerViewModel
    @ViewBuilder let content: Content
    @FocusState var isFocused: Bool
    @State var isSearching: Bool = false
    
    init(filesModel: FilesViewModel, content: @escaping () -> Content) {
        self._model = StateObject(wrappedValue: SearchContainerViewModel(filesModel: filesModel))
        self.content = content()
    }
    
    var body: some View {
        VStack {
            SearchBar(searchContainerModel: model, isFocused: $isFocused)
                .padding(.top)
            
            SearchContainerSubView(isSearching: $isSearching, model: model, dismissSearch: { isFocused = false }, content: content)
                .onChange(of: model.input) { _ in
                    model.search()
                }
        }
        .onChange(of: isFocused) { newValue in
            isSearching = newValue
        }
    }
}

struct SearchBar: View {
    @StateObject var searchContainerModel: SearchContainerViewModel

    @FocusState.Binding var isFocused: Bool
        
    var body: some View {
        HStack {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(.gray)
            
            TextField("Search", text: $searchContainerModel.input)
                .focused($isFocused)
                .onAppear {
                    isFocused = false
                }
                .onExitCommand {
                    searchContainerModel.input = ""
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
                        searchContainerModel.search()
                    }
                })
            
            if isFocused {
                Button(action: {
                    searchContainerModel.input = ""
                    isFocused = false
                }, label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.gray)
                })
                .buttonStyle(.plain)
            }
        }
        .padding(5)
        .modifier(SearchBarSelectionBackgroundModifier(isFocused: $isFocused))
        .padding(.horizontal, 10)
        .padding(.bottom, 10)
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
