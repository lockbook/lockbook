import Foundation
import Bridge

extension Array<LbSearchResult> {
    func toSearchResults() -> [SearchResult] {
        var results: [SearchResult] = []
        
        for result in self {
            if let result = result.doc_result {
                results.append(.document(DocumentSearchResult(result.pointee)))
            } else if let result = result.path_result {
                results.append(.path(PathSearchResult(result.pointee)))
            }
        }
        
        return results
    }
}

public enum SearchResult: Identifiable, Comparable {
    public var id: AnyHashable {
        switch self {
        case .path(let result):
            return result
        case .document(let result):
            return result
        }
    }
    
    public var lbId: UUID {
        switch self {
        case .path(let result):
            return result.id
        case .document(let result):
            return result.id
        }
    }
    
    case path(PathSearchResult)
    case document(DocumentSearchResult)
}

extension Array<LbPathSearchResult> {
    func toPathSearchResults() -> [PathSearchResult] {
        var results: [PathSearchResult] = []
        
        for result in self {
            results.append(PathSearchResult(result))
        }
        
        return results
    }
}

public struct PathSearchResult: Hashable, Comparable {
    public let id: UUID
    public let path: String
    public let score: Int64
    public let matchedIndicies: [UInt]
    
    // For previews
    public init(id: UUID, path: String, score: Int64, matchedIndicies: [UInt]) {
        self.id = id
        self.path = path
        self.score = score
        self.matchedIndicies = matchedIndicies
    }
    
    init(_ res: LbPathSearchResult) {
        self.id = res.id.toUUID()
        self.path = String(cString: res.path)
        self.score = res.score
        self.matchedIndicies = Array(UnsafeBufferPointer(start: res.matched_indicies, count: Int(res.matched_indicies_len)))
    }
        
    public func hash(into hasher: inout Hasher) {
        hasher.combine(id)
        hasher.combine(matchedIndicies)
    }
    
    public static func <(lhs: PathSearchResult, rhs: PathSearchResult) -> Bool {
        if lhs.score == rhs.score {
            return lhs.path < rhs.path
        }
        
        return lhs.score > rhs.score
    }
}

extension Array<LbDocumentSearchResult> {
    func toDocumentSearchResults() -> [DocumentSearchResult] {
        var results: [DocumentSearchResult] = []
        
        for result in self {
            results.append(DocumentSearchResult(result))
        }
        
        return results
    }
}

public struct DocumentSearchResult: Hashable, Comparable {
    public let id: UUID
    public let path: String
    public let contentMatches: [ContentMatch]
    
    init(_ res: LbDocumentSearchResult) {
        self.id = res.id.toUUID()
        self.path = String(cString: res.path)
        self.contentMatches = Array(UnsafeBufferPointer(start: res.content_matches, count: Int(res.content_matches_len))).toContentMatches()
    }
    
    public func hash(into hasher: inout Hasher) {
        hasher.combine(id)
        hasher.combine(contentMatches)
    }
    
    public static func <(lhs: DocumentSearchResult, rhs: DocumentSearchResult) -> Bool {
        let lhsScore = Int(lhs.contentMatches.map({ $0.score }).reduce(0, +)) / lhs.contentMatches.count
        let rhsScore = Int(rhs.contentMatches.map({ $0.score }).reduce(0, +)) / rhs.contentMatches.count
        
        if lhsScore == rhsScore {
            return lhs.path < rhs.path
        }
        
        return lhsScore < rhsScore
    }
}

extension Array<LbContentMatch> {
    func toContentMatches() -> [ContentMatch] {
        var matches: [ContentMatch] = []
        
        for match in self {
            matches.append(ContentMatch(match))
        }
        
        return matches
    }
}

public struct ContentMatch: Hashable {
    public let paragraph: String
    public let score: Int64
    public let matchedIndicies: [UInt]
    
    public func hash(into hasher: inout Hasher) {
        hasher.combine(paragraph)
        hasher.combine(matchedIndicies)
    }

    init(_ match: LbContentMatch) {
        self.paragraph = String(cString: match.paragraph)
        self.score = match.score
        self.matchedIndicies = Array(UnsafeBufferPointer(start: match.matched_indicies, count: Int(match.matched_indicies_len)))
    }
}
