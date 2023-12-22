import Foundation
import SwiftLockbookCore
import CLockbookCore

class SearchService: ObservableObject {
    let core: LockbookApi

    init(_ core: LockbookApi) {
        self.core = core
        
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        decoder.dateDecodingStrategy = .millisecondsSince1970
    }
        
    var pathSearchTask: DispatchWorkItem? = nil
        
    // have bool for search (and have spinner be on top right corner)
    
    @Published var isPathSearching = false
    @Published var isPathAndContentSearching = false
    
    @Published var isPathSearchInProgress = false
    @Published var isPathAndContentSearchInProgress = false
    
    @Published var pathSearchResults: [SearchResult] = []
    @Published var pathAndContentSearchResults: [SearchResult] = []

    @Published var pathSearchSelected = 0
    @Published var pathAndContentSearchSelected = 0
    
    var lastSearchTimestamp = 0
        
    var lastSearchWasComplete = false
    
    let decoder = JSONDecoder()
    
    let updatePathSearchStatus: @convention(c) (UnsafePointer<Int8>?, Int32, UnsafePointer<Int8>?) -> Void = { context, searchResultType, searchResult in
        DispatchQueue.global(qos: .userInitiated).async {
            guard let searchService = UnsafeRawPointer(context)?.load(as: SearchService.self) else {
                return
            }
            
            if !searchService.isPathSearching {
                return
            }
            
            switch searchResultType {
            case 0:
                DispatchQueue.main.sync {
                    searchService.isPathSearchInProgress = true
                    searchService.pathSearchResults.removeAll()
                    searchService.pathSearchSelected = 0
                }
            case 1:
                let data = String(cString: searchResult!).data(using: .utf8)!
                searchService.core.freeText(s: searchResult!)
                
                let nameMatch: FileNameMatch = try! searchService.decoder.decode(FileNameMatch.self, from: data)
                let pathComp = nameMatch.getNameAndPath()
                
                DispatchQueue.main.sync {
                    searchService.pathSearchResults.append(.PathMatch(meta: DI.files.idsAndFiles[nameMatch.id]!, name: pathComp.name, path: pathComp.path, matchedIndices: nameMatch.matchedIndices, score: nameMatch.score))
                    
                    searchService.pathSearchResults.sort { $0.score > $1.score }
                }
            case 2:
                let data = String(cString: searchResult!).data(using: .utf8)!
                searchService.core.freeText(s: searchResult!)
                
                let contentMatches: FileContentMatches = try! searchService.decoder.decode(FileContentMatches.self, from: data)
                let pathComp = contentMatches.getNameAndPath()
                
                
                DispatchQueue.main.sync {
                    for contentMatch in contentMatches.contentMatches {
                        searchService.pathSearchResults.append(.ContentMatch(meta: DI.files.idsAndFiles[contentMatches.id]!, name: pathComp.name, path: pathComp.path, paragraph: contentMatch.paragraph, matchedIndices: contentMatch.matchedIndices, score: contentMatch.score))
                    }
                    
                    searchService.pathSearchResults.sort { $0.score > $1.score }
                }
            case 3:
                DispatchQueue.main.sync {
                    searchService.isPathSearchInProgress = false
                }
            default:
                print("UNRECOGNIZED SEARCH RETURN")
            }
        }
    }
    
    let updatePathAndContentSearchStatus: @convention(c) (UnsafePointer<Int8>?, Int32, UnsafePointer<Int8>?) -> Void = { context, searchResultType, searchResult in
        DispatchQueue.main.async {
            guard let searchService = UnsafeRawPointer(context)?.load(as: SearchService.self) else {
                return
            }
            
            if !searchService.isPathAndContentSearching {
                return
            }
            
            switch searchResultType {
            case 0:
                DispatchQueue.main.sync {
                    searchService.isPathAndContentSearchInProgress = true
                    searchService.pathAndContentSearchResults.removeAll()
                }
            case 1:
                let data = String(cString: searchResult!).data(using: .utf8)!
                searchService.core.freeText(s: searchResult!)
                
                let nameMatch: FileNameMatch = try! searchService.decoder.decode(FileNameMatch.self, from: data)
                let pathComp = nameMatch.getNameAndPath()
                
                DispatchQueue.main.sync {
                    searchService.pathAndContentSearchResults.append(.PathMatch(meta: DI.files.idsAndFiles[nameMatch.id]!, name: pathComp.name, path: pathComp.path, matchedIndices: nameMatch.matchedIndices, score: nameMatch.score))
                    
                    searchService.pathAndContentSearchResults.sort { $0.score > $1.score }
                }
            case 2:
                let data = String(cString: searchResult!).data(using: .utf8)!
                searchService.core.freeText(s: searchResult!)
                
                let contentMatches: FileContentMatches = try! searchService.decoder.decode(FileContentMatches.self, from: data)
                let pathComp = contentMatches.getNameAndPath()
                
                DispatchQueue.main.sync {
                    for contentMatch in contentMatches.contentMatches {
                        searchService.pathAndContentSearchResults.append(.ContentMatch(meta: DI.files.idsAndFiles[contentMatches.id]!, name: pathComp.name, path: pathComp.path, paragraph: contentMatch.paragraph, matchedIndices: contentMatch.matchedIndices, score: contentMatch.score))
                    }
                    
                    searchService.pathAndContentSearchResults.sort { $0.score > $1.score }
                }
            case 3:
                DispatchQueue.main.sync {
                    searchService.isPathAndContentSearchInProgress = false
                }
            default:
                print("UNRECOGNIZED SEARCH RETURN")
            }
        }
    }
    
    func startSearchThread(isPathAndContentSearch: Bool) {
        if !isPathAndContentSearching && isPathAndContentSearch {
            isPathAndContentSearching = true
            isPathAndContentSearchInProgress = true
        } else if !isPathSearching && !isPathAndContentSearch {
            isPathSearching = true
            isPathSearchInProgress = true
        } else {
            return
        }
                
        DispatchQueue.global(qos: .userInitiated).async {
            withUnsafePointer(to: self) { searchServicePtr in
                if case .failure(let err) = self.core.startSearch(isPathAndContentSearch: isPathAndContentSearch, context: searchServicePtr, updateStatus: isPathAndContentSearch ? self.updatePathAndContentSearchStatus : self.updatePathSearchStatus) {
                    DI.errors.handleError(err)
                }
                
                DispatchQueue.main.sync {
                    if isPathAndContentSearch {
                        self.isPathAndContentSearchInProgress = false
                    } else {
                        self.isPathSearchInProgress = false
                    }
                }
            }
        }
    }
    
    func search(query: String, isPathAndContentSearch: Bool) {
        DispatchQueue.global(qos: .userInitiated).async {
            if case .failure(let err) = self.core.searchQuery(query: query, isPathAndContentSearch: isPathAndContentSearch) {
                DI.errors.handleError(err)
            }
        }
    }
      
    func openPathAtIndex(index: Int) {
        if isPathSearching && index < pathSearchResults.count {
            DI.currentDoc.cleanupOldDocs()
            
            DI.currentDoc.openDoc(id: pathSearchResults[index].lbId)
            DI.currentDoc.setSelectedOpenDocById(maybeId: pathSearchResults[index].lbId)
            
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
        if isPathAndContentSearch {
            isPathAndContentSearching = false
            isPathAndContentSearchInProgress = false
            pathAndContentSearchSelected = 0
            pathAndContentSearchResults.removeAll()
        } else {
            isPathSearching = false
            isPathSearchInProgress = false
            pathSearchSelected = 0
            pathSearchResults.removeAll()
        }
        
        if case .failure(let err) = self.core.endSearch(isPathAndContentSearch: isPathAndContentSearch) {
            DI.errors.handleError(err)
        }
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

