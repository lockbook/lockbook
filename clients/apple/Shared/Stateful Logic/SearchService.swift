import Foundation
import SwiftLockbookCore
import CLockbookCore

class SearchService: ObservableObject {
    let core: LockbookApi

    init(_ core: LockbookApi) {
        self.core = core
    }
        
    var pathSearchTask: DispatchWorkItem? = nil
    
    @Published var pathSearchState: SearchState = .NotSearching
    @Published var pathAndContentSearchState: SearchState = .NotSearching
    
    // have bool for search (and have spinner be on top right corner)

    @Published var pathSearchSelected = 0
    var lastSearchTimestamp = 0
        
    var lastSearchWasComplete = false
    
    let decoder = JSONDecoder()
    
    
    let updatePathAndContentSearchStatus: @convention(c) (UnsafePointer<Int8>?, Int32, UnsafePointer<Int8>?) -> Void = { context, searchResultType, searchResult in
        DispatchQueue.main.async {
            guard let searchService = UnsafeRawPointer(context)?.load(as: SearchService.self) else {
                return
            }
            
            let data = String(cString: searchResult!).data(using: .utf8)!
            searchService.core.freeText(s: searchResult!)
            
            var searchResults: [SearchResult] = []
            if case .SearchSuccessful(let originalSearchResults) = searchService.pathAndContentSearchState {
                searchResults = originalSearchResults
            } else {
                searchResults = []
            }
            
            switch searchResultType {
            case 1: // file path match
                let nameMatch: FileNameMatch = try! searchService.decoder.decode(FileNameMatch.self, from: data)
                let pathComp = nameMatch.getNameAndPath()
                
                searchResults.append(.PathMatch(meta: DI.files.idsAndFiles[nameMatch.id]!, name: pathComp.name, path: pathComp.path, matchedIndices: nameMatch.matchedIndices, score: nameMatch.score))
            case 2: // file content match
                let contentMatches: FileContentMatches = try! searchService.decoder.decode(FileContentMatches.self, from: data)
                let pathComp = contentMatches.getNameAndPath()
                
                for contentMatch in contentMatches.contentMatches {
                    searchResults.append(.ContentMatch(meta: DI.files.idsAndFiles[contentMatches.id]!, name: pathComp.name, path: pathComp.path, paragraph: contentMatch.paragraph, matchedIndices: contentMatch.matchedIndices, score: contentMatch.score))
                }
            case 3: // no match
                searchService.pathAndContentSearchState = .NoMatch
                return
            case 4: // new search
                searchService.pathAndContentSearchState = .Searching
                return
            default:
                print("UNRECOGNIZED SEARCH RETURN")
                return
            }
            
            searchResults = searchResults.sorted { $0.score > $1.score }
            if case .Searching = searchService.pathAndContentSearchState {
                searchService.pathAndContentSearchState = .SearchSuccessful(searchResults)
            } else if case .SearchSuccessful(_) = searchService.pathAndContentSearchState {
                searchService.pathAndContentSearchState = .SearchSuccessful(searchResults)
            }
        }
    }
    
    let updatePathSearchStatus: @convention(c) (UnsafePointer<Int8>?, Int32, UnsafePointer<Int8>?) -> Void = { context, searchResultType, searchResult in
        DispatchQueue.main.async {
            guard let searchService = UnsafeRawPointer(context)?.load(as: SearchService.self) else {
                return
            }
            
            let data = String(cString: searchResult!).data(using: .utf8)!
            searchService.core.freeText(s: searchResult!)
            
            var searchResults: [SearchResult] = []
            if case .SearchSuccessful(let originalSearchResults) = searchService.pathSearchState {
                searchResults = originalSearchResults
            } else {
                searchResults = []
            }
            
            switch searchResultType {
            case 1: // file path match
                let nameMatch: FileNameMatch = try! searchService.decoder.decode(FileNameMatch.self, from: data)
                let pathComp = nameMatch.getNameAndPath()
                
                searchResults.append(.PathMatch(meta: DI.files.idsAndFiles[nameMatch.id]!, name: pathComp.name, path: pathComp.path, matchedIndices: nameMatch.matchedIndices, score: nameMatch.score))
            case 3: // no match
                searchService.pathSearchState = .NoMatch
                return
            case 4: // new search
                searchService.pathSearchState = .Searching
                return
            default:
                print("UNRECOGNIZED SEARCH RETURN")
                return
            }
            
            searchResults = Array(searchResults.sorted { $0.score > $1.score }.prefix(20))
            if case .Searching = searchService.pathSearchState {
                searchService.pathSearchState = .SearchSuccessful(searchResults)
            } else if case .SearchSuccessful(_) = searchService.pathSearchState {
                searchService.pathSearchState = .SearchSuccessful(searchResults)
            }
        }
    }
    
    func startPathSearchThread() {
        if case .NotSearching = pathAndContentSearchState {
            endSearch()
        }
    }
    
    func startSearchThread(isPathAndContentSearch: Bool) {
        if case .NotSearching = pathAndContentSearchState {
            print("not already content and path searching")
        } else if case .NotSearching = pathSearchState {
            print("not already path searching")
        } else {
            endSearch()
        }
        
        if isPathAndContentSearch {
            pathAndContentSearchState = .Idle
        } else {
            pathSearchState = .Idle
        }
        
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        decoder.dateDecodingStrategy = .millisecondsSince1970
                
        DispatchQueue.global(qos: .userInitiated).async {
            withUnsafePointer(to: self) { searchServicePtr in
                switch self.core.startSearch(isPathAndContentSearch: isPathAndContentSearch, context: searchServicePtr, updateStatus: isPathAndContentSearch ? self.updatePathAndContentSearchStatus : self.updatePathSearchStatus) {
                case .success(_):
                    print("Finished search")
                case .failure(let err):
                    DI.errors.handleError(err)
                }
            }
        }
    }
    
    func search(query: String) {
        if case .failure(let err) = self.core.searchQuery(query: query) {
            DI.errors.handleError(err)
        }
    }
    
    func endSearch() {
        if case .failure(let err) = self.core.endSearch() {
            DI.errors.handleError(err)
        }
        
        pathAndContentSearchState = .NotSearching
        pathSearchState = .NotSearching
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
    
    func submitSearch(id: UUID) {
        DI.currentDoc.cleanupOldDocs()
        DI.currentDoc.openDoc(id: id)
        DI.currentDoc.setSelectedOpenDocById(maybeId: id)
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

