import Foundation
import SwiftLockbookCore

class SearchService: ObservableObject {
    let core: LockbookApi

    init(_ core: LockbookApi) {
        self.core = core
    }
    
    @Published var isSearching: Bool = false
    @Published var searchResults: [SearchResultItem] = []
    
    func searchFilePath(input: String) -> [SearchResultItem]? {
        switch core.searchFilePaths(input: input) {
        case .success(let paths):
            return paths
        case .failure(let err):
            DI.errors.handleError(err)
            return nil
        }
    }
    
    func startSearch() {
        isSearching = true
    }
    
    func submitSearch(id: UUID) {
        DI.currentDoc.selectedDocument = DI.files.idsAndFiles[id]!
    }
}
