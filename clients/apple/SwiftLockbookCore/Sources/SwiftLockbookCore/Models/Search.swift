import Foundation

public struct SearchResultItem: Identifiable, Codable, Hashable {
    public var id: UUID
    public var path: String
    public var score: Int64
    public var matchedIndices: [Int]
}

extension String {
    public func getNameAndPath() -> (name: String, path: String) {
        let components = self.split(separator: "/")
        
        let name = String(components.last ?? "ERROR")
        let path = components.dropLast().joined(separator: "/")

        
        return (name, path)
    }
}

public struct FileNameMatch: Decodable, Identifiable {
    public var id = UUID()
    
    public var path: String
    public var matchedIndices: [Int]
    public var score: Int
    
    public func getFormattedNameAndPath() -> (name: String, path: String) {
        var formattedFullPath = path
        
        for index in (0...matchedIndices.count - 1).reversed() {
            let correctIndex = formattedFullPath.index(formattedFullPath.startIndex, offsetBy: matchedIndices[index])
            
            formattedFullPath.replaceSubrange(correctIndex...correctIndex, with: "**\(formattedFullPath[correctIndex])**")
        }

        print("OLD PATH: \(path) and NEW PATH: \(formattedFullPath)")
        
        let components = formattedFullPath.split(separator: "/")
        
        let name = String(components.last ?? "ERROR")
        let path = components.dropLast().joined(separator: "/")
                
        return (name, path)
    }
}

public struct FileContentMatches: Decodable {
    public var id: UUID
    public var path: String
    public var contentMatches: [ContentMatch]
    
    public func getNameAndPath() -> (name: String, path: String) {
        let components = path.split(separator: "/")
        
        let name = String(components.last ?? "ERROR")
        let path = components.dropLast().joined(separator: "/")
        
        return (name, path)
    }
}

public struct ContentMatch: Decodable {
    public var paragraph: String
    public var matchedIndices: [Int]
    public var score: Int
    
    public func getFormattedParagraph() -> String {
        var formattedParagraph = paragraph
        
        for index in (0...matchedIndices.count - 1).reversed() {
            let correctIndex = formattedParagraph.index(formattedParagraph.startIndex, offsetBy: matchedIndices[index])
            
            formattedParagraph.replaceSubrange(correctIndex...correctIndex, with: "**\(formattedParagraph[correctIndex])**")
        }

        return formattedParagraph
    }
}

public struct NoMatch {
    public init() {}
}
