import Foundation
import SwiftLockbookCore
import CLockbookCore

class SearchService: ObservableObject {
    let core: LockbookApi

    init(_ core: LockbookApi) {
        self.core = core
    }
        
    var pathSearchTask: DispatchWorkItem? = nil
    @Published var pathSearchState: SearchPathState = .NotSearching
    @Published var pathSearchSelected = 0
    var lastSearchTimestamp = 0
    
    @Published var pathsSearchResult: Array<FilePathInfo> = []
    @Published var searchPathAndContentState: SearchPathAndContentState = .NotSearching
    
    func startSearchThread() {
        searchPathAndContentState = .Idle
        
        DispatchQueue.global(qos: .userInitiated).async {
            withUnsafePointer(to: self) { searchServicePtr in
                switch self.core.startSearch(context: searchServicePtr, updateStatus: { context, searchResultType, searchResult in
                    DispatchQueue.main.sync {
                        let decoder = JSONDecoder()
                        decoder.keyDecodingStrategy = .convertFromSnakeCase
                        decoder.dateDecodingStrategy = .millisecondsSince1970
                        
                        guard let searchService = UnsafeRawPointer(context)?.load(as: SearchService.self) else {
                            return
                        }
                        
                        let data = String(cString: searchResult!).data(using: .utf8)!
                        searchService.core.freeText(s: searchResult!)
                        
                        var searchResults: [SearchResult] = []
                        if case .SearchSuccessful(let originalSearchResults) = searchService.searchPathAndContentState {
                            searchResults = originalSearchResults
                        } else {
                            searchResults = []
                        }
                        
                        switch searchResultType {
                        case 1: // file path match
                            let nameMatch: FileNameMatch = try! decoder.decode(FileNameMatch.self, from: data)
                            let pathComp = nameMatch.getNameAndPath()
                            
                            searchResults.append(.PathMatch(meta: DI.files.idsAndFiles[nameMatch.id]!, name: pathComp.name, path: pathComp.path, matchedIndices: nameMatch.matchedIndices, score: nameMatch.score))
                        case 2: // file content match
                            let contentMatches: FileContentMatches = try! decoder.decode(FileContentMatches.self, from: data)
                            let pathComp = contentMatches.getNameAndPath()
                            
                            for contentMatch in contentMatches.contentMatches {
                                searchResults.append(.ContentMatch(meta: DI.files.idsAndFiles[contentMatches.id]!, name: pathComp.name, path: pathComp.path, paragraph: contentMatch.paragraph, matchedIndices: contentMatch.matchedIndices, score: contentMatch.score))
                            }
                        case 3: // no match
                            searchService.searchPathAndContentState = .NoMatch
                            return
                        default:
                            print("UNRECOGNIZED SEARCH RETURN")
                            return
                        }
                        
                        searchResults = searchResults.sorted { $0.score > $1.score }
                        if case .Searching = searchService.searchPathAndContentState {
                            searchService.searchPathAndContentState = .SearchSuccessful(searchResults)
                        } else if case .SearchSuccessful(_) = searchService.searchPathAndContentState {
                            searchService.searchPathAndContentState = .SearchSuccessful(searchResults)
                        }
                    }
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
        
    func searchFilePath(input: String) -> [SearchResultItem]? {
        switch core.searchFilePaths(input: input) {
        case .success(let paths):
            return paths
        case .failure(let err):
            DI.errors.handleError(err)
            return nil
        }
    }
    
    func asyncSearchFilePath(input: String) {
        self.pathSearchState = .Searching
        self.pathSearchSelected = 0
        
        pathSearchTask?.cancel()
        
        lastSearchTimestamp = Int(Date().timeIntervalSince1970 * 1_000)
        let currentSearchTimestamp = lastSearchTimestamp
        
        let newPathSearchTask = DispatchWorkItem {
            let result = self.core.searchFilePaths(input: input)
            
            DispatchQueue.main.async {
                switch result {
                case .success(let paths):
                    if currentSearchTimestamp == self.lastSearchTimestamp && self.pathSearchState != .NotSearching {
                        self.pathSearchState = .SearchSuccessful(paths)
                    }
                case .failure(let err):
                    self.pathSearchState = .Idle
                    DI.errors.handleError(err)
                }
            }
        }
        
        DispatchQueue.global(qos: .userInitiated).asyncAfter(deadline: .now() + .milliseconds(550), execute: newPathSearchTask)
        
        pathSearchTask = newPathSearchTask
    }
    
    func openPathAtIndex(index: Int) {
        if case .SearchSuccessful(let paths) = pathSearchState,
           index < paths.count {
            DI.currentDoc.cleanupOldDocs()

            DI.currentDoc.openDoc(id: paths[index].id)
            DI.currentDoc.setSelectedOpenDocById(maybeId: paths[index].id)
            
            pathSearchState = .NotSearching
            pathSearchSelected = 0
        }
    }
    
    func selectNextPath() {
        if case .SearchSuccessful(let paths) = pathSearchState {
            pathSearchSelected = min(paths.count - 1, pathSearchSelected + 1)
        }
    }
    
    func selectPreviousPath() {
        pathSearchSelected = max(0, pathSearchSelected - 1)
    }
    
    func startPathSearch() {
        pathSearchState = .Idle
    }
    
    func submitSearch(id: UUID) {
        DI.currentDoc.cleanupOldDocs()
        DI.currentDoc.openDoc(id: id)
        DI.currentDoc.setSelectedOpenDocById(maybeId: id)
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
        case .PathMatch(let id, _, _, _, _, _):
            return id
        case .ContentMatch(let id, _, _, _, _, _, _):
            return id
        }
    }
    
    public var score: Int {
        switch self {
        case .PathMatch(_, _, _, _, _, let score):
            return score
        case .ContentMatch(_, _, _, _, _, _, let score):
            return score
        }
        
    }
    
    case PathMatch(id: UUID = UUID(), meta: File, name: String, path: String, matchedIndices: [Int], score: Int)
    case ContentMatch(id: UUID = UUID(), meta: File, name: String, path: String, paragraph: String, matchedIndices: [Int], score: Int)
}

public enum SearchPathAndContentState {
    case NotSearching
    case Idle
    case Searching
    case NoMatch
    case SearchSuccessful([SearchResult])
}

public enum SearchPathState: Equatable {
    case NotSearching
    case Idle
    case Searching
    case SearchSuccessful([SearchResultItem])
}

