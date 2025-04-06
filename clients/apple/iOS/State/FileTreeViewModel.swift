import SwiftUI
import SwiftWorkspace
import Combine

class FileTreeViewModel: ObservableObject {
    @Published var openFolders: Set<UUID> = Set()
    @Published var openDoc: UUID? = nil
    
    private var cancellables: Set<AnyCancellable> = []
    
    let filesModel: FilesViewModel
    
    init(filesModel: FilesViewModel) {
        self.filesModel = filesModel
        
        AppState.workspaceState.$openDoc.sink { [weak self] openDoc in
            self?.openDoc = openDoc
        }
        .store(in: &cancellables)
    }
    
    func toggleFolder(_ id: UUID) {
        if self.openFolders.remove(id) == id {
            return
        }
        
        openFolders.insert(id)
        print("added \(id)")
    }
        
    func getParents(_ file: File) -> [UUID] {
        var parents: [UUID] = []
        
        guard case .success(var current) = AppState.lb.getFile(id: file.parent) else {
            return []
        }
        
        while current.id != current.parent {
            parents.append(current.id)
            
            if case let .success(newCurrent) = AppState.lb.getFile(id: current.parent) {
                current = newCurrent
            } else {
                return parents
            }
        }
        
        return parents
    }
}
