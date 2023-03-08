import Foundation
import SwiftLockbookCore

class SearchService: ObservableObject {
    let core: LockbookApi

    init(_ core: LockbookApi) {
        self.core = core
    }
    
    @Published var isSearching: Bool = false
    @Published var searchResults: [SearchResultItem] = []
    
    func searchFilePath(input: String) {
        switch core.searchFilePaths(input: input) {
        case .success(let paths):
            searchResults = paths
        case .failure(let err):
            DI.errors.handleError(err)
        }
    }
    
    func startSearch() {
        isSearching = true
    }
}
