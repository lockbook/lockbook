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
        
    func startSearchThread(isPathAndContentSearch: Bool) {
        let searchPaths = true
        var searchDocs = false

        if !isPathAndContentSearching && isPathAndContentSearch {
            searchDocs = true
            isPathAndContentSearching = true
        } else if !isPathSearching && !isPathAndContentSearch {
            isPathSearching = true
        } else {
            return
        }

        DispatchQueue.global(qos: .userInitiated).async {
            switch self.core.search(input: "", searchPaths: searchPaths, searchDocs: searchDocs) {
            case .success(let results):
                DispatchQueue.main.async {
                    if isPathAndContentSearch {
                        self.pathAndContentSearchResults = results
                    } else {
                        self.pathSearchResults = results
                    }
                }
            case .failure(let err):
                print("i do nothing for now")
            }
            
        }
        
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
            print("returned early! \(isPathAndContentSearch) \(isPathAndContentSearching) \(isPathSearching)")
            return
        }
        
        DispatchQueue.global(qos: .userInitiated).async {
            switch self.core.search(input: query, searchPaths: searchPaths, searchDocs: searchDocs) {
            case .success(let results):
                DispatchQueue.main.async {
                    if isPathAndContentSearch {
                        self.pathAndContentSearchResults = results
                        self.isPathAndContentSearchInProgress = false
                    } else {
                        self.pathSearchResults = results
                        self.isPathSearchInProgress = false
                    }
                }
            case .failure(let err):
                print("i do nothing for now")
            }
            
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
    }
}

struct FilePathInfo: Identifiable {
    let id = UUID()
    
    let meta: File
    let searchResult: SearchResult
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

