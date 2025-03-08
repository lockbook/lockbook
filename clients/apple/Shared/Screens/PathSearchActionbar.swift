import SwiftUI
import SwiftWorkspace

struct PathSearchActionbar<Content: View>: View {
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
                            
                            if model.isSearchInProgress {
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
                ForEach(Array(model.results.enumerated()), id: \.element.id) {index, result in
                    SearchResultCellView(name: result.path.nameAndPath().0, path: result.path.nameAndPath().1, matchedIndices: result.matchedIndicies, index: index)
                }
                .scrollIndicators(.visible)
                .padding(.horizontal)
            }
            .onChange(of: model.selected) { newValue in
                withAnimation {
                    if newValue < model.results.count {
                        scrollHelper.scrollTo(model.results[newValue].id, anchor: .center)
                    }
                }
            }
        }
        .frame(maxHeight: 500)
    }
}

struct SearchResultCellView: View {
    @EnvironmentObject var pathSearchModel: PathSearchViewModel
    
    let name: String
    let path: String
    let matchedIndices: [UInt]
    
    let index: Int
    
    @State var pathModified: Text = Text("")
    @State var nameModified: Text = Text("")
    
    var body: some View {
        Button(action: {
            pathSearchModel.selected = index
            pathSearchModel.openSelected()
        }, label: {
            HStack {
                Image(systemName: FileIconHelper.docNameToSystemImageName(name: name))
                    .resizable()
                    .frame(width: 20, height: 25)
                    .padding(.horizontal, 10)
                    .foregroundColor(index == pathSearchModel.selected ? .white : .primary)
                
                VStack(alignment: .leading) {
                    nameModified
                        .font(.system(size: 16))
                        .multilineTextAlignment(.leading)
                        .foregroundColor(index == pathSearchModel.selected ? .white : .primary)
                    
                    pathModified
                        .multilineTextAlignment(.leading)
                        .foregroundColor(index == pathSearchModel.selected ? .white : .gray)
                }
                
                Spacer()
                
                if index == pathSearchModel.selected {
                    Text("↩")
                        .padding(.horizontal)
                        .foregroundColor(index == pathSearchModel.selected ? .white : .gray)
                } else if index < 9 {
                    Text("⌘\(index + 1)")
                        .padding(.horizontal)
                        .foregroundColor(index == pathSearchModel.selected ? .white : .gray)
                }
            }
            .frame(height: 40)
            .padding(EdgeInsets(top: 4, leading: 0, bottom: 4, trailing: 0))
            .contentShape(Rectangle())
            .onAppear {
                underlineMatchedSegments()
            }
            .background(index == pathSearchModel.selected ? RoundedRectangle(cornerSize: CGSize(width: 5, height: 5)).foregroundColor(.blue.opacity(0.8)) : RoundedRectangle(cornerSize: CGSize(width: 5, height: 5)).foregroundColor(.clear))
        })
    }
    
    func underlineMatchedSegments() {
        let matchedIndicesHash = Set(matchedIndices)
        
        var pathOffset = 1;
        
        if(path.count - 1 > 0) {
            pathModified = Text("")
            
            for index in 0...path.count - 1 {
                let correctIndex = String.Index(utf16Offset: index, in: path)
                let newPart = Text(path[correctIndex...correctIndex])
                
                if(matchedIndicesHash.contains(UInt(index + 1))) {
                    pathModified = pathModified + newPart.bold()
                } else {
                    pathModified = pathModified + newPart
                }
            }
            
            pathOffset = 2
        }
                
        if(name.count - 1 > 0) {
            nameModified = Text("")
            for index in 0...name.count - 1 {
                let correctIndex = String.Index(utf16Offset: index, in: name)
                let newPart = Text(name[correctIndex...correctIndex])
                
                if(matchedIndicesHash.contains(UInt(index + path.count + pathOffset))) {
                    nameModified = nameModified + newPart.bold()
                } else {
                    nameModified = nameModified + newPart
                }
            }
        }
    }
}
