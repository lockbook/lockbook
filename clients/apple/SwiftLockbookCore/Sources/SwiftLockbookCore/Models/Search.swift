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
