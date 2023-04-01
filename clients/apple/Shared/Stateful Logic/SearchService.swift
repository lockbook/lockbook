import Foundation
import SwiftLockbookCore

class SearchService: ObservableObject {
    let core: LockbookApi

    init(_ core: LockbookApi) {
        self.core = core
    }
    
    @Published var isSearching: Bool = false
    @Published var filePathInfos: Array<FilePathInfo> = []
    
    #if os(iOS)
    
    func asyncSearchFilePath(input: String) {
        DispatchQueue.main.async {
            print("SEARCHING \(input)")
            switch self.core.searchFilePaths(input: input) {
            case .success(let results):
                print("POSTING \(input)")
                self.filePathInfos = results.map() { result in
                    FilePathInfo(meta: DI.files.idsAndFiles[result.id]!, searchResult: result)
                }
            case .failure(let err):
                DI.errors.handleError(err)
            }
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
    
    func startSearch() {
        isSearching = true
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
