import Bridge
import Foundation

extension [LbSearchResult] {
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
        case let .path(result):
            result
        case let .document(result):
            result
        }
    }

    public var lbId: UUID {
        switch self {
        case let .path(result):
            result.id
        case let .document(result):
            result.id
        }
    }

    case path(PathSearchResult)
    case document(DocumentSearchResult)
}

extension [LbPathSearchResult] {
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

    /// For previews
    public init(id: UUID, path: String, score: Int64, matchedIndicies: [UInt]) {
        self.id = id
        self.path = path
        self.score = score
        self.matchedIndicies = matchedIndicies
    }

    init(_ res: LbPathSearchResult) {
        id = res.id.toUUID()
        path = String(cString: res.path)
        score = res.score
        matchedIndicies = Array(UnsafeBufferPointer(start: res.matched_indicies, count: Int(res.matched_indicies_len)))
    }

    public func hash(into hasher: inout Hasher) {
        hasher.combine(id)
        hasher.combine(matchedIndicies)
    }

    public static func < (lhs: PathSearchResult, rhs: PathSearchResult) -> Bool {
        if lhs.score == rhs.score {
            return lhs.path < rhs.path
        }

        return lhs.score > rhs.score
    }
}

extension [LbDocumentSearchResult] {
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
        id = res.id.toUUID()
        path = String(cString: res.path)
        contentMatches = Array(UnsafeBufferPointer(start: res.content_matches, count: Int(res.content_matches_len))).toContentMatches()
    }

    public func hash(into hasher: inout Hasher) {
        hasher.combine(id)
        hasher.combine(contentMatches)
    }

    public static func < (lhs: DocumentSearchResult, rhs: DocumentSearchResult) -> Bool {
        let lhsScore = Int(lhs.contentMatches.map(\.score).reduce(0, +)) / lhs.contentMatches.count
        let rhsScore = Int(rhs.contentMatches.map(\.score).reduce(0, +)) / rhs.contentMatches.count

        if lhsScore == rhsScore {
            return lhs.path < rhs.path
        }

        return lhsScore < rhsScore
    }
}

extension [LbContentMatch] {
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
        paragraph = String(cString: match.paragraph)
        score = match.score
        matchedIndicies = Array(UnsafeBufferPointer(start: match.matched_indicies, count: Int(match.matched_indicies_len)))
    }
}

public protocol PathSearching: AnyObject {
    func query(_ input: String) -> [PathSearcherResult]
}

public struct PathSearcherResult: Hashable, Identifiable {
    public let id: UUID
    public let filename: String
    public let parentPath: String
    public let matchedIndices: [UInt]

    public init(id: UUID, filename: String, parentPath: String, matchedIndices: [UInt]) {
        self.id = id
        self.filename = filename
        self.parentPath = parentPath
        self.matchedIndices = matchedIndices
    }

    init(_ res: LbPathSearcherResult) {
        id = res.id.toUUID()
        filename = String(cString: res.filename)

        // Strip leading `/` so filename highlight offsets line up with full-path indices.
        let rawParent = String(cString: res.parent_path)
        parentPath = rawParent == "/" ? "/" : String(rawParent.dropFirst())

        matchedIndices = Array(
            UnsafeBufferPointer(start: res.matched_indices, count: Int(res.matched_indices_len))
        ).map { UInt($0) }
    }
}

public final class LbPathSearcher: PathSearching {
    private let handle: OpaquePointer

    init(lb: OpaquePointer?) {
        handle = lb_path_searcher_new(lb)!
    }

    deinit {
        lb_free_path_searcher(handle)
    }

    public func query(_ input: String) -> [PathSearcherResult] {
        let res = lb_path_searcher_query(handle, input)
        defer { lb_free_path_search_results(res) }

        guard let ptr = res.results else { return [] }
        return Array(UnsafeBufferPointer(start: ptr, count: Int(res.results_len)))
            .map(PathSearcherResult.init)
    }
}

public final class MockPathSearcher: PathSearching {
    public init() {}
    public func query(_: String) -> [PathSearcherResult] { [] }
}
