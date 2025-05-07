import SwiftUI
import SwiftWorkspace

struct PathSearchContainerView<Content: View>: View {
    @StateObject var model = PathSearchViewModel()
    @ViewBuilder var content: Content
    
    #if os(macOS)
    @FocusState var focused: Bool
    #endif
    
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
                .onTapGesture {
                    model.endSearch()
                }
                .foregroundColor(.gray.opacity(0.01))
            
            GeometryReader { geometry in
                VStack {
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
                            
                        } else if !model.isSearchInProgress && !model.results.isEmpty {
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
                }
                .padding(.top, geometry.size.height / 4.5)
                .padding(.leading, (geometry.size.width / 2) - 250)
                .padding(.bottom, 100)
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

#Preview {
    var pathSearchModel = PathSearchViewModel()
    pathSearchModel.isShown = true
    
    return PathSearchContainerView(model: pathSearchModel) {
        Color.red
    }
}
