import SwiftUI
import SwiftWorkspace
import Combine

class FileTreeViewModel: ObservableObject {
    @Published var openFiles: Set<UUID> = Set()
    @Published var implicitlyOpenFiles: Set<UUID> = Set()
    @Published var selectedDoc: UUID? = nil
    
    @Published var sheetInfo: FileOperationSheetInfo? = nil
    
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
        
        guard case .success(var current) = MainState.lb.getFile(id: file.parent) else {
            return []
        }
        
        while current.id != current.parent {
            parents.append(current.id)
            
            if case let .success(newCurrent) = MainState.lb.getFile(id: current.parent) {
                current = newCurrent
            } else {
                return parents
            }
        }
        
        return parents
    }
}

struct CreateFolderSheetInfo: Identifiable {
    var parent: File
    var id: UUID {
        parent.id
    }
}


enum FileOperationSheetInfo: Identifiable {
    case create(parent: File)
    case rename(file: File)
    case select(action: SelectFolderAction)
    case share(file: File)
    
    var id: String {
        switch self {
        case .create(let parent):
            return "create-\(parent.id)"
        case .rename(let file):
            return "rename-\(file.id)"
        case .select(let action):
            return "select-\(action.id)"
        case .share(let file):
            return "share-\(file.id)"
        }
    }
}

enum SelectFolderAction {
    case Move(ids: [UUID])
    case Import(paths: [String])
    case AcceptShare(name: String, id: UUID)
    
    var id: String {
        switch self {
        case .Move(let ids):
            return "move-\(ids.map(\.uuidString).joined(separator: ","))"
        case .Import(let paths):
            return "import-\(paths.joined(separator: ","))"
        case .AcceptShare(let name, let id):
            return "acceptShare-\(name)-\(id.uuidString)"
        }
    }

}
