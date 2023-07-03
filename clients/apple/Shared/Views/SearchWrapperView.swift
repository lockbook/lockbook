import SwiftUI
import SwiftLockbookCore

struct SearchWrapperView<Content: View>: View {
    @EnvironmentObject var search: SearchService
    @EnvironmentObject var fileService: FileService
    @EnvironmentObject var current: DocumentService
    
    @Environment(\.isSearching) var isSearching
    
    @Binding var searchInput: String
    
    var mainView: Content
    var isiOS: Bool
    
    var body: some View {
        VStack {
            switch search.searchPathAndContentState {
            case .NotSearching:
                mainView
            case .Idle:
                #if os(iOS)
                Spacer()
                #else
                mainView
                #endif
            case .NoMatch:
                Spacer()
                Text("No search results")
                Spacer()
            case .Searching:
                Spacer()
                ProgressView()
                Spacer()
            case .SearchSuccessful(let results):
                if !isiOS {
                    List(results) { result in
                        switch result {
                        case .PathMatch(_, let meta, let name, let path, let matchedIndices, _):
                            Button(action: {
                                current.openDocuments[meta.id] = DocumentLoadingInfo(meta)
                            }) {
                                SearchFilePathCell(name: name, path: path, matchedIndices: matchedIndices)
                            }
                        case .ContentMatch(_, let meta, let name, let path, let paragraph, let matchedIndices, _):
                            Button(action: {
                                current.openDocuments[meta.id] = DocumentLoadingInfo(meta)
                            }) {
                                SearchFileContentCell(name: name, path: path, paragraph: paragraph, matchedIndices: matchedIndices)
                            }
                        }
                        
#if os(macOS)
                        Divider()
#endif
                    }
                    .setiPadOrMacOSSearchListStyle()
                } else {
#if os(iOS)
                    List(results) { result in
                        switch result {
                        case .PathMatch(_, let meta, let name, let path, let matchedIndices, _):
                            NavigationLink(destination: DocumentView(model: current.openDoc(meta: meta))) {
                                SearchFilePathCell(name: name, path: path, matchedIndices: matchedIndices)
                            }
                        case .ContentMatch(_, let meta, let name, let path, let paragraph, let matchedIndices, _):
                            NavigationLink(destination: DocumentView(model: current.openDoc(meta: meta))) {
                                SearchFileContentCell(name: name, path: path, paragraph: paragraph, matchedIndices: matchedIndices)
                            }
                        }
                    }
                    .listStyle(.insetGrouped)
#endif
                }
            }
        }
        .onChange(of: searchInput) { newInput in
            if (!newInput.isEmpty) {
                search.search(query: newInput)
            } else {
                search.searchPathAndContentState = .Idle
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

extension List {
    func setiPadOrMacOSSearchListStyle() -> some View {
        #if os(iOS)
        self.listStyle(.inset)
        #else
        self
            .listStyle(.automatic)
        #endif
    }
}

extension VStack {
    func setiOSOrMacOSSearchPadding() -> some View {
        #if os(iOS)
        self.padding(.vertical, 5)
        #else
        self
        #endif
    }
}

struct SearchFilePathCell: View {
    let name: String
    let path: String
    let matchedIndices: [Int]
    
    @State var nameModified: Text
    @State var pathModified: Text
    
    init(name: String, path: String, matchedIndices: [Int]) {
        self.name = name
        self.path = path
        self.matchedIndices = matchedIndices
        
        let nameAndPath = SearchFilePathCell.underlineMatchedSegments(name: name, path: path, matchedIndices: matchedIndices)
        
        self._nameModified = State(initialValue: nameAndPath.formattedName)
        self._pathModified = State(initialValue: nameAndPath.formattedPath)
    }
        
    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            nameModified
                .font(.title3)
            
            HStack {
                Image(systemName: "doc")
                    .foregroundColor(.accentColor)
                    .font(.caption)
                
                pathModified
                    .foregroundColor(.blue)
                    .font(.caption)
            }
        }
            .setiOSOrMacOSSearchPadding()
            .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
    }
    
    static func underlineMatchedSegments(name: String, path: String, matchedIndices: [Int]) -> (formattedName: Text, formattedPath: Text) {
        let matchedIndicesHash = Set(matchedIndices)
        var pathOffset = 1;
        var formattedPath = Text("")
        
        if(path.count - 1 > 0) {
            for index in 0...path.count - 1 {
                let correctIndex = String.Index(utf16Offset: index, in: path)
                let newPart = Text(path[correctIndex...correctIndex])
                                
                if(path[correctIndex...correctIndex] == "/") {
                    formattedPath = formattedPath + Text(" > ").foregroundColor(.gray)
                } else if(matchedIndicesHash.contains(index + 1)) {
                    formattedPath = formattedPath + newPart.bold()
                } else {
                    formattedPath = formattedPath + newPart
                }
            }
            
            pathOffset = 2
        }
        
        var formattedName = Text("")
                
        if(name.count - 1 > 0) {
            for index in 0...name.count - 1 {
                let correctIndex = String.Index(utf16Offset: index, in: name)
                let newPart = Text(name[correctIndex...correctIndex])
                
                if(matchedIndicesHash.contains(index + path.count + pathOffset)) {
                    formattedName = formattedName + newPart.bold()
                } else {
                    formattedName = formattedName + newPart.foregroundColor(.gray)
                }
            }
        }
        
        return (formattedName, formattedPath)
    }
}

struct SearchFileContentCell: View {
    let name: String
    let path: String
    let paragraph: String
    let matchedIndices: [Int]
    
    @State var formattedParagraph: Text
    @State var formattedPath: Text
    
    init(name: String, path: String, paragraph: String, matchedIndices: [Int]) {
        self.name = name
        self.path = path
        self.paragraph = paragraph
        self.matchedIndices = matchedIndices
        
        let pathAndParagraph = SearchFileContentCell.underlineMatchedSegments(path: path, paragraph: paragraph, matchedIndices: matchedIndices)
        
        self._formattedPath = State(initialValue: pathAndParagraph.formattedPath)
        self._formattedParagraph = State(initialValue: pathAndParagraph.formattedParagraph)
    }
    
    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(name)
                .font(.title3)
                .foregroundColor(.gray)
            
            HStack {
                Image(systemName: "doc")
                    .foregroundColor(.accentColor)
                    .font(.caption2)
                
                formattedPath
                    .foregroundColor(.accentColor)
                    .font(.caption2)
            }
            .padding(.bottom, 7)
            
            formattedParagraph
                .font(.caption)
                .lineLimit(nil)
        }
        .setiOSOrMacOSSearchPadding()
        .contentShape(Rectangle()) /// https://stackoverflow.com/questions/57258371/swiftui-increase-tap-drag-area-for-user-interaction
    }
    
    static func underlineMatchedSegments(path: String, paragraph: String, matchedIndices: [Int]) -> (formattedPath: Text, formattedParagraph: Text) {
        let matchedIndicesHash = Set(matchedIndices)
        
        var formattedPath = Text("")
        
        if(path.count - 1 > 0) {
            for index in 0...path.count - 1 {
                let correctIndex = String.Index(utf16Offset: index, in: path)
                                
                if(path[correctIndex...correctIndex] == "/") {
                    formattedPath = formattedPath + Text(" > ").foregroundColor(.gray)
                } else {
                    formattedPath = formattedPath + Text(path[correctIndex...correctIndex])
                }
            }
        }
        
        var formattedParagraph = Text("")
                
        if(paragraph.count - 1 > 0) {
            for index in 0...paragraph.count - 1 {
                let correctIndex = String.Index(utf16Offset: index, in: paragraph)
                let newPart = Text(paragraph[correctIndex...correctIndex])
                
                if(matchedIndicesHash.contains(index)) {
                    formattedParagraph = formattedParagraph + newPart.bold()
                } else {
                    formattedParagraph = formattedParagraph + newPart.foregroundColor(.gray)
                }
            }
        }
        
        return (formattedPath, formattedParagraph)
    }
}

