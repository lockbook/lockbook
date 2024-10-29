import Foundation
import SwiftWorkspace

class SearchService: ObservableObject {
    let core: Lb

    init(_ core: Lb) {
        self.core = core
    }
        
    var pathSearchTask: DispatchWorkItem? = nil
    
    @Published var isPathSearching = false
    @Published var isPathAndContentSearching = false
    
    @Published var isPathSearchInProgress = false
    @Published var isPathAndContentSearchInProgress = false
    
    @Published var pathSearchResults: [SearchResult] = []
    @Published var pathAndContentSearchResults: [SearchResult] = []

    @Published var pathSearchSelected = 0
        
    var pathSearchQuery = ""
    var pathAndContentSearchQuery = ""
        
    func startSearchThread(searchPaths: Bool, searchDocs: Bool) {
        core.search(input: "", searchPaths: searchPaths, searchDocs: searchDocs)
    }
    
    func search(query: String, isPathAndContentSearch: Bool) {
        let searchPaths = true
        var searchDocs = false
        
        if isPathAndContentSearch && isPathAndContentSearching {
            self.isPathAndContentSearchInProgress = true
            searchDocs = true
            self.pathAndContentSearchQuery = query
        } else if !isPathAndContentSearch && isPathSearching {
            self.isPathSearchInProgress = true
            self.pathSearchQuery = query
        } else {
            return
        }
        
        switch self.core.search(input: query, searchPaths: searchPaths, searchDocs: searchDocs) {
        case .success((let pathResults, let docResults)):
            print("i do nothing... for now")
        case .failure(let err):
            print("i do nothing...")
        }
    }
      
    func openPathAtIndex(index: Int) {
        if isPathSearching && index < pathSearchResults.count {
            DI.workspace.requestOpenDoc(pathSearchResults[index].lbId)
            
            endSearch(isPathAndContentSearch: false)
        }
    }
    
    func selectNextPath() {
        if isPathSearching && pathSearchResults.count > 0 {
            pathSearchSelected = min(pathSearchResults.count - 1, pathSearchSelected + 1)
        }
    }
    
    func selectPreviousPath() {
        pathSearchSelected = max(0, pathSearchSelected - 1)
    }

    func endSearch(isPathAndContentSearch: Bool) {
        if isPathAndContentSearch && isPathAndContentSearching {
            isPathAndContentSearching = false
            pathAndContentSearchQuery = ""
            isPathAndContentSearchInProgress = false
            pathAndContentSearchResults.removeAll()
        } else if !isPathAndContentSearch && isPathSearching {
            isPathSearching = false
            pathSearchQuery = ""
            isPathSearchInProgress = false
            pathSearchSelected = 0
            pathSearchResults.removeAll()
        } else {
            return
        }
        
        DI.workspace.shouldFocus = true
        
//        if case .failure(let err) = self.core.endSearch(isPathAndContentSearch: isPathAndContentSearch) {
//            DI.errors.handleError(err)
//        }
    }
}

struct FilePathInfo: Identifiable {
    let id = UUID()
    
    let meta: File
    let searchResult: SearchResult
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
    
    public var lbId: UUID {
        switch self {
        case .PathMatch(_, let meta, _, _, _, _):
            return meta.id
        case .ContentMatch(_, let meta, _, _, _, _, _):
            return meta.id
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

public enum SearchState {
    case NotSearching
    case Idle
    case Searching
    case NoMatch
    case SearchSuccessful([SearchResult])
    
    func isSearching() -> Bool {
        if case .NotSearching = self {
            false
        } else {
            true
        }
    }
}

