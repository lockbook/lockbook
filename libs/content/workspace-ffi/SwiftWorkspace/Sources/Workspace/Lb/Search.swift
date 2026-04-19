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

public protocol ContentSearching: AnyObject {
    func query(_ input: String) -> [ContentSearcherResult]
    func snippet(id: UUID, match: ContentSearcherMatch, contextChars: Int) -> SearcherSnippet?
}

public struct ContentSearcherMatch: Hashable {
    public let rangeStart: Int
    public let rangeEnd: Int
    public let exact: Bool

    public init(rangeStart: Int, rangeEnd: Int, exact: Bool) {
        self.rangeStart = rangeStart
        self.rangeEnd = rangeEnd
        self.exact = exact
    }

    init(_ m: LbContentSearcherMatch) {
        rangeStart = Int(m.range_start)
        rangeEnd = Int(m.range_end)
        exact = m.exact
    }
}

public struct ContentSearcherResult: Hashable, Identifiable {
    public let id: UUID
    public let filename: String
    public let parentPath: String
    public let matches: [ContentSearcherMatch]

    public init(id: UUID, filename: String, parentPath: String, matches: [ContentSearcherMatch]) {
        self.id = id
        self.filename = filename
        self.parentPath = parentPath
        self.matches = matches
    }

    init(_ res: LbContentSearcherResult) {
        id = res.id.toUUID()
        filename = String(cString: res.filename)

        let rawParent = String(cString: res.parent_path)
        parentPath = rawParent == "/" ? "/" : String(rawParent.dropFirst())

        let matchesPtr: UnsafeBufferPointer<LbContentSearcherMatch>
        if let ptr = res.matches {
            matchesPtr = UnsafeBufferPointer(start: ptr, count: Int(res.matches_len))
        } else {
            matchesPtr = UnsafeBufferPointer(start: nil, count: 0)
        }
        matches = matchesPtr.map(ContentSearcherMatch.init)
    }
}

public struct SearcherSnippet: Hashable {
    public let prefix: String
    public let matched: String
    public let suffix: String
}

/// Thin wrapper over the Rust `ContentSearcher`. Not thread-safe on its own — callers must
/// serialize access (e.g. via a dedicated dispatch queue). Calling `query` and `snippet`
/// concurrently from different threads is undefined behavior.
public final class LbContentSearcher: ContentSearching {
    private let handle: OpaquePointer

    init(lb: OpaquePointer?) {
        handle = lb_content_searcher_new(lb)!
    }

    deinit {
        lb_free_content_searcher(handle)
    }

    public func query(_ input: String) -> [ContentSearcherResult] {
        let res = lb_content_searcher_query(handle, input)
        defer { lb_free_content_search_results(res) }

        guard let ptr = res.results else { return [] }
        return Array(UnsafeBufferPointer(start: ptr, count: Int(res.results_len)))
            .map(ContentSearcherResult.init)
    }

    public func snippet(id: UUID, match: ContentSearcherMatch, contextChars: Int) -> SearcherSnippet? {
        let res = lb_content_searcher_snippet(
            handle, id.toLbUuid(), UInt(match.rangeStart), UInt(match.rangeEnd), UInt(contextChars)
        )
        defer { lb_free_content_searcher_snippet(res) }

        guard let prefix = res.prefix, let matched = res.matched, let suffix = res.suffix else {
            return nil
        }
        return SearcherSnippet(
            prefix: Self.clean(String(cString: prefix)),
            matched: Self.clean(String(cString: matched)),
            suffix: Self.clean(String(cString: suffix))
        )
    }

    /// Flatten newlines so the snippet fits on a single line. Matches the cleanup the egui
    /// search UI does in `workspace::search::content::extract_snippet`.
    private static func clean(_ s: String) -> String {
        String(s.map { ($0 == "\n" || $0 == "\r") ? " " : $0 })
    }
}

public final class MockContentSearcher: ContentSearching {
    public init() {}
    public func query(_: String) -> [ContentSearcherResult] { [] }
    public func snippet(id _: UUID, match _: ContentSearcherMatch, contextChars _: Int) -> SearcherSnippet? { nil }
}
