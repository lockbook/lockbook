import Foundation

class SelectFolderViewModel: ObservableObject {
    @Published var searchInput: String = ""
    @Published var error: String? = nil
    
    @Published var folderPaths: [String]? = nil
    var filteredFolderPaths: [String] {
        if let folderPaths = folderPaths {
            if searchInput.isEmpty {
                return folderPaths
            } else {
                return folderPaths.filter { path in
                    path.localizedCaseInsensitiveContains(searchInput)
                }
            }
        } else {
            return []
        }
    }
    
    @Published var selected = 0
    var selectedPath: String {
        get {
            if filteredFolderPaths.count <= selected {
                return ""
            }
            
            return filteredFolderPaths[selected].isEmpty ? "/" : filteredFolderPaths[selected]
        }
    }
    
    var exit: Bool = false
    
    func calculateFolderPaths() {
        DispatchQueue.global(qos: .userInitiated).async {
            switch DI.core.listFolderPaths() {
            case .success(let paths):
                DispatchQueue.main.async {
                    self.folderPaths = paths.map({ String($0.dropFirst()) }).sorted()
                }
            case .failure(_):
                DispatchQueue.main.async {
                    self.error = "Could not get folder paths."
                }
            }
        }
    }
    
    func selectFolder(action: SelectFolderAction, path: String) -> Bool {
        switch DI.core.getByPath(path: path) {
        case .success(let parent):
            return selectFolder(action: action, newParent: parent.id)
        case .failure(let err):
            error = err.msg
            
            return false
        }
    }
    
    func selectFolder(action: SelectFolderAction, newParent: UUID) -> Bool {
        switch action {
        case .Move(let ids):
            for id in ids {
                
                if case .failure(let err) = DI.core.moveFile(id: id, newParent: newParent) {
                    error = err.msg
                    return false
                }
            }
            
            DI.files.successfulAction = .move
            DI.files.refresh()
            DI.selected.selectedFiles = nil
            
            return true
        case .Import(let paths):
            if case .failure(let err) = DI.core.importFiles(sources: paths, dest: newParent) {
                error = err.msg
                
                return false
            }
            
            DI.files.successfulAction = .importFiles
            DI.files.refresh()
            DI.selected.selectedFiles = nil
            
            return true
        case .AcceptShare((let name, let id)):
            if case .failure(let err) = DI.core.createLink(name: name, parent: newParent, target: id) {
                error = err.msg
                return false
            }
            
            DI.files.successfulAction = .acceptedShare
            DI.files.refresh()
            DI.share.calculatePendingShares()
            DI.selected.selectedFiles = nil
            
            return true
        }
    }
}

enum SelectFolderAction {
    case Move([UUID])
    case Import([String])
    case AcceptShare((String, UUID))
}

enum SelectFolderMode {
    case List
    case Tree
}
