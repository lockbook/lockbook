import SwiftUI
import SwiftWorkspace
import Combine

class FileTreeViewModel: ObservableObject {
    @Published var openFiles: Set<UUID> = Set()
    @Published var implicitlyOpenFiles: Set<UUID> = Set()
    @Published var selectedDoc: UUID? = nil
        
    private var cancellables: Set<AnyCancellable> = []
    
    init(workspaceState: WorkspaceState) {
        workspaceState.$openDoc.sink { [weak self] selectedDoc in
            self?.selectedDoc = selectedDoc
        }
        .store(in: &cancellables)
    }
    
    func openFile(_ file: File) {
        if(self.openFiles.contains(file.id)) {
            return
        }
        
        openFiles.insert(file.id)
        implicitlyOpenFiles.formUnion(self.getParents(file))
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
