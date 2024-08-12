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
        switch DI.core.getFileByPath(path: path) {
        case .success(let parent):
            print("got the folder id selected: \(path) to \(parent.id)")
            return selectFolder(action: action, newParent: parent.id)
        case .failure(let cError):
            error = cError.description
            
            return false
        }
    }
    
    func selectFolder(action: SelectFolderAction, newParent: UUID) -> Bool {
        switch action {
        case .Move(let ids):
            for id in ids {
                
                if case .failure(let cError) = DI.core.moveFile(id: id, newParent: newParent) {
                    switch cError.kind {
                    case .UiError(.FolderMovedIntoItself):
                        error = "You cannot move a folder into itself."
                    case .UiError(.InsufficientPermission):
                        error = "You do not have the permission to do that."
                    case .UiError(.LinkInSharedFolder):
                        error = "You cannot move a link into a shared folder."
                    case .UiError(.TargetParentHasChildNamedThat):
                        error = "A child with that name already exists in that folder."
                    default:
                        error = cError.description
                    }

                    return false
                }
            }
            
            DI.files.successfulAction = .move
            DI.files.refresh()
            
            return true
        case .Import(let paths):
            if case .failure(let cError) = DI.core.importFiles(sources: paths, destination: newParent) {
                error = cError.description
                
                return false
            }
            
            DI.files.successfulAction = .importFiles
            DI.files.refresh()
            
            return true
        case .AcceptShare((let name, let id)):
            if case .failure(let cError) = DI.core.createLink(name: name, dirId: newParent, target: id) {
                error = cError.description
                
                return false
            }
            
            DI.files.successfulAction = .acceptedShare
            DI.files.refresh()
            DI.share.calculatePendingShares()
            
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
