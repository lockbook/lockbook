import Foundation

public struct SearchResultItem: Identifiable, Codable, Hashable {
    public var id: UUID
    public var path: String
    public var score: Int64
    public var matchedIndices: [Int]
    
    public func getNameAndPath() -> (name: String, path: String) {
        let components = path.split(separator: "/")
        
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
    
    public func getNameAndPath() -> (name: String, path: String) {        
        let components = path.split(separator: "/")
        
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
}

public struct NoMatch {
    public init() {}
}
