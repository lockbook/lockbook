import Foundation
import Bridge

extension Array<LbPathSearchResult> {
    func toPathSearchResults() -> [PathSearchResult] {
        var results: [PathSearchResult] = []
        
        for result in self {
            results.append(PathSearchResult(result))
        }
        
        return results
    }
}

public struct PathSearchResult {
    let id: UUID
    let path: String
    let score: Int64
    let matchedIndicies: [UInt]
    
    init(_ res: LbPathSearchResult) {
        self.id = res.id.toUUID()
        self.path = String(cString: res.path)
        self.score = res.score
        self.matchedIndicies = Array(UnsafeBufferPointer(start: res.matched_indicies, count: Int(res.matched_indicies_len)))
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

public struct DocumentSearchResult {
    let id: UUID
    let path: String
    let contentMatches: [ContentMatch]
    
    init(_ res: LbDocumentSearchResult) {
        self.id = res.id.toUUID()
        self.path = String(cString: res.path)
        self.contentMatches = Array(UnsafeBufferPointer(start: res.content_matches, count: Int(res.content_matches_len))).toContentMatches()
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

public struct ContentMatch {
    let paragraph: String
    let score: Int64
    let matchedIndicies: [UInt]
    
    init(_ match: LbContentMatch) {
        self.paragraph = String(cString: match.paragraph)
        self.score = match.score
        self.matchedIndicies = Array(UnsafeBufferPointer(start: match.matched_indicies, count: Int(match.matched_indicies_len)))
    }
}
