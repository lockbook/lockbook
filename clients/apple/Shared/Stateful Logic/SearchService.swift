import Foundation
import SwiftLockbookCore

class SearchService: ObservableObject {
    let core: LockbookApi

    init(_ core: LockbookApi) {
        self.core = core
    }
    
    @Published var isPathSearching: Bool = false
    @Published var pathsSearchResult: Array<FilePathInfo> = []
    @Published var searchPathAndContentState: SearchPathAndContentState = .NotSearching
    
    #if os(iOS)
    
    func asyncSearchFilePath(input: String) {
        DispatchQueue.main.async {
            switch self.core.searchFilePaths(input: input) {
            case .success(let results):
                self.pathsSearchResult = results.map() { result in
                    FilePathInfo(meta: DI.files.idsAndFiles[result.id]!, searchResult: result)
                }
            case .failure(let err):
                DI.errors.handleError(err)
            }
        }
    }
    
    func startSearchThread() {
        searchPathAndContentState = .Searching
        
        DispatchQueue.global(qos: .userInitiated).async {
            withUnsafePointer(to: self) { searchServicePtr in
                switch self.core.startSearch(context: searchServicePtr, updateStatus: { context, searchResultType, searchResult in
                    let decoder = JSONDecoder()
                    decoder.keyDecodingStrategy = .convertFromSnakeCase
                    decoder.dateDecodingStrategy = .millisecondsSince1970
                    
                    guard let searchService = UnsafeRawPointer(context)?.load(as: SearchService.self) else {
                        return
                    }
                                        
                    let data = String(cString: searchResult!).data(using: .utf8)!
                    
                    var searchResults: [SearchResult] = []
                    if case .SearchSuccessful(let originalSearchResults) = searchService.searchPathAndContentState {
                        searchResults = originalSearchResults
                    } else {
                        searchResults = []
                    }
                    
                    switch searchResultType {
                    case 1: // file path match
                        let nameMatch: FileNameMatch = try! decoder.decode(FileNameMatch.self, from: data)
                        let pathComp = nameMatch.path.getNameAndPath()
                        
                        searchResults.append(.PathMatch(meta: DI.files.idsAndFiles[nameMatch.id]!, name: pathComp.name, path: pathComp.path, score: nameMatch.score, matchedIndices: nameMatch.matchedIndices))
                    case 2: // file content match
                        let contentMatches: FileContentMatches = try! decoder.decode(FileContentMatches.self, from: data)
                        let pathComp = contentMatches.path.getNameAndPath()
                        
                        for contentMatch in contentMatches.contentMatches {
                            searchResults.append(.ContentMatch(meta: DI.files.idsAndFiles[contentMatches.id]!, name: pathComp.name, path: pathComp.path, contentMatch: contentMatch))
                        }
                    case 3: // no match
                        searchService.searchPathAndContentState = .NoMatch
                        return
                    default:
                        print("UNRECOGNIZED SEARCH RETURN")
                        return
                    }
                    
                    searchResults = searchResults.sorted { $0.score > $1.score }
                    searchService.searchPathAndContentState = .SearchSuccessful(searchResults)
                }) {
                case .success(_):
                    print("Finished search")
                case .failure(let err):
                    DI.errors.handleError(err)
                }
            }
            
        }
    }
    
    func search(query: String) {
        searchPathAndContentState = .Searching
        
        if case .failure(let err) = self.core.searchQuery(query: query) {
            DI.errors.handleError(err)
        }
    }
    
    func endSearch() {
        if case .failure(let err) = self.core.endSearch() {
            DI.errors.handleError(err)
        }
        
        searchPathAndContentState = .NotSearching
    }
    
    #endif
    
    func searchFilePath(input: String) -> [SearchResultItem]? {
        switch core.searchFilePaths(input: input) {
        case .success(let paths):
            return paths
        case .failure(let err):
            DI.errors.handleError(err)
            return nil
        }
    }
    
    func startPathSearch() {
        isPathSearching = true
    }
    
    func submitSearch(id: UUID) {
        DI.currentDoc.selectedDocument = DI.files.idsAndFiles[id]!
    }
}

struct FilePathInfo: Identifiable {
    let id = UUID()
    
    let meta: File
    let searchResult: SearchResultItem
}

public enum SearchResult: Identifiable {
    public var id: UUID {
        switch self {
        case .PathMatch(let meta, _, _, _, _):
            return meta.id
        case .ContentMatch(let meta, _, _, _):
            return meta.id
        }
    }
    
    public var score: Int {
        switch self {
        case .PathMatch(_, _, _, let score, _):
            return score
        case .ContentMatch(_, _, _, let contentMatch):
            return contentMatch.score
        }
        
    }
    
    case PathMatch(meta: File, name: String, path: String, score: Int, matchedIndices: [Int])
    case ContentMatch(meta: File, name: String, path: String, contentMatch: ContentMatch)
}

public enum SearchPathAndContentState {
    case NotSearching
    case Searching
    case NoMatch
    case SearchSuccessful([SearchResult])
}

