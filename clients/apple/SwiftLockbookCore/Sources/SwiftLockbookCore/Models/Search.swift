import Foundation

public struct SearchResultItem: Identifiable, Codable, Hashable {
    public var id: UUID
    public var path: String
    public var score: Int64
    public var matchedIndices: [Int]
}

extension String {
    public func getNameAndPath() -> (name: String, path: String){
        let components = self.split(separator: "/")
        
        let name = String(components.last ?? "ERROR")
        let path = components.dropLast().joined(separator: "/")

        
        return (name, path)
    }
}

//public protocol PathAndContentSearchResult {
//
//}

public struct FileNameMatch: Decodable {
    public var id: UUID
    public var path: String
    public var matchedIndices: [Int]
    public var score: Int64
    
    public init() {
        self.id = UUID()
        self.path = "/personal/money/bitcoin.md"
        self.matchedIndices = [2, 5, 6, 7, 8]
        self.score = 2
    }
}

public struct FileContentMatches: Decodable {
    public var id: UUID
    public var path: String
    public var contentMatches: [ContentMatch]
}

public struct ContentMatch: Decodable {
    public var paragraph: String
    public var matchedIndices: [Int]
    public var score: Int
}

public struct NoMatch {
    public init() {
        
    }
}

public enum PathAndContentSearchResult {
    case FileNameMatch(FileNameMatch)
    case FileContentMatches(FileContentMatches)
    case NoMatch(NoMatch)
}

