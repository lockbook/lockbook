import SwiftUI
import SwiftWorkspace

class PathSearchViewModel: ObservableObject {
    @Published var input: String = ""
    @Published var isShown: Bool = false
    @Published var isSearchInProgress: Bool = false
    
    @Published var results: [PathSearchResult] = []
    @Published var selected = 0
    
    let filesModel: FilesViewModel
    
    init(filesModel: FilesViewModel) {
        self.filesModel = filesModel
    }
    
    func openSelected() {
        guard selected != -1 || (selected == -1 && !results.isEmpty) else {
            return
        }
        
        guard selected < results.count else {
            return
        }
                
        guard let file = filesModel.idsToFiles[results[selected].id] else {
            return
        }
        
        if(file.type == .folder) {
            AppState.workspaceState.selectedFolder = file.id
        } else {
            AppState.workspaceState.requestOpenDoc(file.id)
        }
        
        self.isShown = false
    }
    
    func search() {
        selected = 0
        
        DispatchQueue.global(qos: .userInitiated).async {
            let res = AppState.lb.search(input: self.input, searchPaths: true, searchDocs: false)
            
            DispatchQueue.main.async {
                switch res {
                case .success(let results):
                    self.results = results.map({
                        switch $0 {
                        case .document(_):
                            return nil
                        case .path(let pathSearchResult):
                            return pathSearchResult
                        }
                    }).compactMap({ $0 }).prefix(20).sorted()
                                        
                    self.selected = min(self.selected, results.count - 1)
                case .failure(let err):
                    print("got error: \(err.msg)")
                }
            }
        }
    }
    
    func selectNextPath() {
        if results.count > 0 {
            self.selected = min(results.count - 1, selected + 1)
        }
    }
    
    func selectPreviousPath() {
        self.selected = max(0, selected - 1)
    }
    
    func endSearch() {
        self.isShown = false
    }
}
