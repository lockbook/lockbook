import SwiftUI
import SwiftWorkspace
import Combine

class WrappedWorkspaceState: ObservableObject {
    let homeState: HomeState
    let filesModel: FilesViewModel
    
    private var cancellables: Set<AnyCancellable> = []
    
    init(homeState: HomeState, filesModel: FilesViewModel) {
        self.homeState = homeState
        self.filesModel = filesModel
        
        AppState.workspaceState.$renameOpenDoc.sink { [weak self] rename in
            self?.runOnActiveWorkspaceState(doRun: rename) { file in
                DispatchQueue.main.async {
                    self?.homeState.sheetInfo = .rename(file: file)
                }
            }
        }
        .store(in: &cancellables)
        
        AppState.workspaceState.$newFolderButtonPressed.sink { [weak self] newFolder in
            guard newFolder else {
                return
            }
            
            guard let root = self?.filesModel.root else {
                return
            }
            
            DispatchQueue.main.async {
                homeState.sheetInfo = .createFolder(parent: root)
            }
        }
        .store(in: &cancellables)
    }
    
    func runOnActiveWorkspaceState(doRun: Bool, f: @escaping (File) -> Void) {
        guard let openDoc = AppState.workspaceState.openDoc else {
            return
        }
        
        if doRun {
            if let file = filesModel.idsToFiles[openDoc] {
                f(file)
            }
        }
    }
}
