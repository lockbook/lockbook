import Foundation
import SwiftLockbookCore

class SearchService: ObservableObject {
    let core: LockbookApi

    init(_ core: LockbookApi) {
        self.core = core
    }
    
    @Published var isPathSearching: Bool = false
    @Published var isPathAndContentSearching: Bool = false
    @Published var pathsSearchResult: Array<FilePathInfo> = []
    @Published var pathsAndContentSearchResult: Array<PathAndContentSearchResult> = []
    
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
                    
                    switch searchResultType {
                    case 1: // file path match
                        searchService.pathsAndContentSearchResult.append(.FileNameMatch(try! decoder.decode(FileNameMatch.self, from: data)))
                    case 2: // file content match
                        searchService.pathsAndContentSearchResult.append(.FileContentMatches(try! decoder.decode(FileContentMatches.self, from: data)))
                    case 3: // no match
                        searchService.pathsAndContentSearchResult.append(.NoMatch(NoMatch()))
                    default:
                        print("UNRECOGNIZED SEARCH RETURN")
                        return
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
        pathsAndContentSearchResult.removeAll()
        
        if case .failure(let err) = self.core.searchQuery(query: query) {
            DI.errors.handleError(err)
        }
    }
    
    func endSearch() {
        if case .failure(let err) = self.core.endSearch() {
            DI.errors.handleError(err)
        }
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
    
    func startPathAndContentSearch() {
        isPathAndContentSearching = true
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
