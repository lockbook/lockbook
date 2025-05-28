import SwiftUI
import SwiftWorkspace

struct SearchPathResultView: View {
    let name: String
    let path: String
    let matchedIndices: [UInt]
    
    @State var nameModified: Text
    @State var pathModified: Text
    
    init(name: String, path: String, matchedIndices: [UInt]) {
        self.name = name
        self.path = path
        self.matchedIndices = matchedIndices
        
        let nameAndPath = SearchPathResultView.underlineMatchedSegments(name: name, path: path, matchedIndices: matchedIndices)
        
        self._nameModified = State(initialValue: nameAndPath.formattedName)
        self._pathModified = State(initialValue: nameAndPath.formattedPath)
    }
        
    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            HStack {
                nameModified
                    .font(.title3)
                
                Spacer()
            }
            
            HStack {
                Image(systemName: "doc")
                    .foregroundColor(.accentColor)
                    .font(.caption)
                
                pathModified
                    .foregroundColor(Color.accentColor)
                    .font(.caption)
                
                Spacer()
            }
        }
            .padding(.vertical, 5)
            .contentShape(Rectangle())
    }
    
    static func underlineMatchedSegments(name: String, path: String, matchedIndices: [UInt]) -> (formattedName: Text, formattedPath: Text) {
        let matchedIndicesHash = Set(matchedIndices)
        var pathOffset = 1;
        var formattedPath = Text("")
        
        if(path.count - 1 > 0) {
            for index in 0...path.count - 1 {
                let correctIndex = String.Index(utf16Offset: index, in: path)
                let newPart = Text(path[correctIndex...correctIndex])
                                
                if(path[correctIndex...correctIndex] == "/") {
                    formattedPath = formattedPath + Text(" > ").foregroundColor(.gray)
                } else if(matchedIndicesHash.contains(UInt(index + 1))) {
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
                
                if(matchedIndicesHash.contains(UInt(index + path.count + pathOffset))) {
                    formattedName = formattedName + newPart.bold()
                } else {
                    formattedName = formattedName + newPart.foregroundColor(.gray)
                }
            }
        }
        
        return (formattedName, formattedPath)
    }
}

struct SearchContentResultView: View {
    let name: String
    let path: String
    let contentMatches: [ContentMatch]
    
    @State var formattedParagraphs: [(AnyHashable, Text)]
    @State var formattedPath: Text
    
    init(name: String, path: String, contentMatches: [ContentMatch]) {
        self.name = name
        self.path = path
        self.contentMatches = contentMatches
        
        let pathAndParagraphs = SearchContentResultView.underlineMatchedSegments(path: path, contentMatches: contentMatches)
        
        self._formattedPath = State(initialValue: pathAndParagraphs.formattedPath)
        self._formattedParagraphs = State(initialValue: pathAndParagraphs.formattedParagraphs)
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
                    .multilineTextAlignment(.leading)
                
                Spacer()
            }
            .padding(.bottom, 7)
            
            ForEach(formattedParagraphs, id: \.0) { (_, paragraph) in
                HStack {
                    paragraph
                        .font(.caption)
                        .lineLimit(nil)
                        .multilineTextAlignment(.leading)
                }
                .padding(.bottom, 7)
            }
        }
        .padding(.vertical, 5)
        .contentShape(Rectangle())
    }
    
    static func underlineMatchedSegments(path: String, contentMatches: [ContentMatch]) -> (formattedPath: Text, formattedParagraphs: [(AnyHashable, Text)]) {
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
        
        var formattedParagraphs: [(AnyHashable, Text)] = []
        
        for contentMatch in contentMatches {
            formattedParagraphs.append((contentMatch, Self.underlineParagraph(paragraph: contentMatch.paragraph, matchedIndices: contentMatch.matchedIndicies)))
        }
        
        return (formattedPath, formattedParagraphs)
    }
    
    static func underlineParagraph(paragraph: String, matchedIndices: [UInt]) -> Text {
        let matchedIndicesHash = Set(matchedIndices)
        var formattedParagraph = Text("")
                
        if(paragraph.count - 1 > 0) {
            for index in 0...paragraph.count - 1 {
                let correctIndex = String.Index(utf16Offset: index, in: paragraph)
                let newPart = Text(paragraph[correctIndex...correctIndex])
                
                if(matchedIndicesHash.contains(UInt(index))) {
                    formattedParagraph = formattedParagraph + newPart.bold()
                } else {
                    formattedParagraph = formattedParagraph + newPart.foregroundColor(.gray)
                }
            }
        }
        
        return formattedParagraph
    }
}

struct PathSearchResultView: View {
    let name: String
    let path: String
    let matchedIndices: [UInt]
    
    var index: Int = -1
    var isSelected: Bool = false
    
    @State var pathModified: Text = Text("")
    @State var nameModified: Text = Text("")
    
    var body: some View {
        HStack {
            Image(systemName: FileIconHelper.docNameToSystemImageName(name: name))
                .resizable()
                .aspectRatio(contentMode: .fit)
                .frame(width: 20, height: 25)
                .padding(.horizontal, 10)
                .foregroundColor(isSelected ? .white : .primary)
            
            VStack(alignment: .leading) {
                nameModified
                    .font(.system(size: 16))
                    .multilineTextAlignment(.leading)
                    .foregroundColor(isSelected ? .white : .primary)
                
                pathModified
                    .multilineTextAlignment(.leading)
                    .foregroundColor(isSelected ? .white : .gray)
            }
            
            Spacer()
            
            if isSelected {
                Text("↩")
                    .padding(.horizontal)
                    .foregroundColor(isSelected ? .white : .gray)
            } else if index < 9 {
                Text("⌘\(index + 1)")
                    .padding(.horizontal)
                    .foregroundColor(isSelected ? .white : .gray)
            }
        }
        .frame(height: 40)
        .padding(EdgeInsets(top: 4, leading: 0, bottom: 4, trailing: 0))
        .contentShape(Rectangle())
        .onAppear {
            underlineMatchedSegments()
        }
        .background(isSelected ? RoundedRectangle(cornerSize: CGSize(width: 5, height: 5)).foregroundColor(Color.accentColor.opacity(0.8)) : RoundedRectangle(cornerSize: CGSize(width: 5, height: 5)).foregroundColor(.clear))
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

#Preview("Path Search Result - Selected") {
    PathSearchResultView(name: "secrets.md", path: "/my/super/secret/path", matchedIndices: [23, 24, 25])
}

#Preview("Path Search Result - Selected") {
    PathSearchResultView(name: "my-other-secrets.md", path: "/my/super/secret/path", matchedIndices: [23, 24, 25], isSelected: true)
}
