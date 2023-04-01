import Foundation

public struct SearchResultItem: Identifiable, Codable, Hashable {
    public var id: UUID
    public var path: String
    public var score: Int64
    public var matchedIndices: [Int]
}

extension SearchResultItem {
    public func getNameAndPath() -> (name: String, path: String){
        let components = path.split(separator: "/")
        
        let name = String(components.last ?? "ERROR")
        let path = components.dropLast().joined(separator: "/")

        
        return (name, path)
    }
}

public struct FileNameMatch {
    public var id: UUID
    public var path: String
    public var matched_indices: [Int]
    public var score: Int64
}

public struct FileContentMatches {
    public var id: UUID
    public var path: String
    public var content_matches: [ContentMatch]
}

public struct ContentMatch {
    public var paragraph: String
    public var matchedIndices: [Int]
    public var score: Int64
}

public struct NoMatch {}
