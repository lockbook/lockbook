import SwiftUI
import SwiftWorkspace
import Combine

class FileTreeViewModel: ObservableObject {
    @Published var openFiles: Set<UUID> = Set()
    @Published var implicitlyOpenFiles: Set<UUID> = Set()
    @Published var selectedDoc: UUID? = nil
    
    @Published var sheetInfo: FileOperationSheetInfo? = nil
    @Published var selectSheetInfo: SelectFolderAction? = nil
    
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

struct CreateFolderSheetInfo: Identifiable {
    var parent: File
    var id: UUID {
        parent.id
    }
}


enum FileOperationSheetInfo: Identifiable {
    case createFolder(parent: File)
    case rename(file: File)
    case share(file: File)
    
    var id: String {
        switch self {
        case .createFolder(let parent):
            return "createFolder-\(parent.id)"
        case .rename(let file):
            return "rename-\(file.id)"
//        case .select(let action):
//            return "select-\(action.id)"
        case .share(let file):
            return "share-\(file.id)"
        }
    }
}

enum SelectFolderAction: Identifiable {
    case move(files: [File])
    case externalImport(paths: [String])
    case acceptShare(name: String, id: UUID)
    
    var id: String {
        switch self {
        case .move(let ids):
            return "move-\(ids.map(\.name).joined(separator: ","))"
        case .externalImport(let paths):
            return "import-\(paths.joined(separator: ","))"
        case .acceptShare(let name, let id):
            return "acceptShare-\(name)-\(id.uuidString)"
        }
    }

}
