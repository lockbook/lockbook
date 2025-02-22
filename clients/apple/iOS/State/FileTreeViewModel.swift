import SwiftUI
import SwiftWorkspace
import Combine

class FileTreeViewModel: ObservableObject {
    @Published var openFolders: Set<UUID> = Set()
    @Published var selectedDoc: UUID? = nil
    
    private var cancellables: Set<AnyCancellable> = []
    
    let filesModel: FilesViewModel
    
    init(workspaceState: WorkspaceState, filesModel: FilesViewModel) {
        self.filesModel = filesModel
        
        workspaceState.$openDoc.sink { [weak self] selectedDoc in
            self?.selectedDoc = selectedDoc
        }
        .store(in: &cancellables)
    }
    
    func toggleFolder(_ id: UUID) {
        if self.openFolders.remove(id) == id {
            return
        }
        
        openFolders.insert(id)
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

enum SelectedFilesState {
    case unselected
    case selected(explicitly: Set<File>, implicitly: Set<File>)
    
    var count: Int {
        get {
            switch self {
                
            case .unselected:
                return 0
            case .selected(explicitly: let explicitly, implicitly: _):
                return explicitly.count
            }
        }
    }
    
    func isSelected(_ file: File) -> Bool {
        switch self {
        case .unselected:
            return false
        case .selected(explicitly: _, implicitly: let implcitly):
            return implcitly.contains(file)
        }
    }
    
    func isSelectableState() -> Bool {
        switch self {
        case .unselected:
            return false
        case .selected(explicitly: _, implicitly: _):
            return true
        }
    }
}
